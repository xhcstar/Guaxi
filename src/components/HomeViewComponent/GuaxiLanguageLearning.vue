<template>
  <n-message-provider>
    <div class="lang-learn-page">
      <div class="manager-header">
        <div class="title-main">学习记录管理器</div>
        <div class="toolbar-row">
          <n-button quaternary circle size="small" title="听力资源管理" @click="openAudioManager">
            <Icon :icon="settingsIcon" width="16" height="16" />
          </n-button>
          <n-button quaternary circle size="small" title="导入并恢复" :loading="importingLearningData" @click="importLearningData">
            <Icon :icon="importIcon" width="16" height="16" />
          </n-button>
          <n-button quaternary circle size="small" title="导出学习数据" :loading="exportingLearningData" @click="exportLearningData">
            <Icon :icon="exportIcon" width="16" height="16" />
          </n-button>
          <n-button quaternary circle size="small" title="随机抽查卡牌" @click="openRandomCheckBoard">
            <Icon :icon="shuffleIcon" width="16" height="16" />
          </n-button>
          <n-tag v-if="syncQueueVisualRunning" size="small" type="warning">后台转换 {{ Math.min(syncQueueDone, syncQueueTotal) }}/{{ syncQueueTotal }}</n-tag>
          <n-select
            v-model:value="learningLang"
            size="small"
            :options="learningLangOptions"
            style="width: 140px"
          />
          <n-input
            v-model:value="keyword"
            clearable
            size="small"
            placeholder="搜索单词、释义、例句、笔记..."
            style="width: 260px"
            @keydown.enter="refreshAll"
          />
          <n-button quaternary circle size="small" title="刷新数据" @click="refreshAll">
            <Icon :icon="refreshIcon" width="16" height="16" />
          </n-button>
        </div>
      </div>

      <n-tabs v-model:value="activeTab" animated type="line" class="page-tabs">
        <n-tab-pane name="manager" tab="学习管理">
          <div v-if="isListeningImmersiveReady" class="listening-immersive-page">
            <div class="immersive-global-back">
              <n-button quaternary circle size="small" class="immersive-back-btn" title="返回听力管理界面" @click="exitListeningImmersive">
                <Icon :icon="chevronLeftIcon" width="16" height="16" />
              </n-button>
              <span class="immersive-title">{{ activeAudio?.title || '听力学习面板' }}</span>
            </div>
            <div class="listening-detail-layout immersive">
              <div class="listening-player-pane">
                <div class="immersive-media-shell">
                  <video
                    v-if="activeMediaIsVideo"
                    ref="activeAudioPlayerRef"
                    class="listening-player listening-video-player"
                    controls
                    playsinline
                    :src="activeAudioPlayableSrc"
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
                    :src="activeAudioPlayableSrc"
                    @loadedmetadata="onAudioLoadedMetadata"
                    @canplay="handleListeningMediaCanPlay('active', $event)"
                    @play="handleListeningMediaPlay('active', $event)"
                    @timeupdate="handleAudioTimeUpdate"
                    @error="handleListeningMediaError('active', $event)"
                  ></audio>
                </div>
                <div class="playback-row">
                  <span class="detail-label">播放速度</span>
                  <n-select v-model:value="playbackRate" size="small" style="width: 140px" :options="playbackRateOptions" @update:value="applyAudioPlaybackRate" />
                </div>
                <div class="detail-block">
                  <div class="detail-label">资源来源 / 主题</div>
                  <div class="detail-content">{{ activeAudio?.provider || '—' }} / {{ activeAudio?.topic || '—' }}</div>
                </div>
                <div class="detail-actions">
                  <n-button size="small" @click="editAudio(activeAudio)">编辑资源</n-button>
                  <n-button size="small" @click="applyReviewResult('known')">听懂了</n-button>
                  <n-button size="small" @click="applyReviewResult('fuzzy')">半懂</n-button>
                  <n-button size="small" @click="applyReviewResult('unknown')">听不懂</n-button>
                </div>
              </div>
              <div class="listening-transcript-pane">
                <div class="transcript-head">
                  <div class="detail-label">音文对照</div>
                  <div class="transcript-head-actions">
                    <n-tag v-if="activeTranscriptTranslating" size="small" type="warning">译文加载中...</n-tag>
                    <n-button quaternary circle size="small" class="transcript-mode-btn" :title="`切换显示模式（当前：${transcriptDisplayModeLabel}）`" @click="cycleTranscriptDisplayMode">
                      <Icon :icon="languagesIcon" width="13" height="13" />
                    </n-button>
                    <n-button quaternary circle size="small" class="transcript-mode-btn" :disabled="!activeMediaIsVideo" :title="`切换视频字幕（当前：${subtitleDisplayModeLabel}）`" @click="cycleSubtitleDisplayMode">
                      <Icon :icon="badgeInfoIcon" width="13" height="13" />
                    </n-button>
                    <n-button quaternary circle size="small" class="transcript-font-btn" title="减小字号" @click="decreaseTranscriptFontScale">A-</n-button>
                    <n-button quaternary circle size="small" class="transcript-font-btn" title="增大字号" @click="increaseTranscriptFontScale">A+</n-button>
                  </div>
                </div>
                <div ref="immersiveTranscriptListRef" class="transcript-list" :style="transcriptListStyle" @scroll="handleActiveTranscriptScroll">
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
                    <div class="transcript-source" v-if="transcriptDisplayMode !== 'target'">{{ item.line.text }}</div>
                    <div class="transcript-target" v-if="transcriptDisplayMode !== 'source'">{{ resolveTranscriptTargetText(item.line) }}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div v-else class="manager-layout" :class="{ 'left-collapsed': leftCollapsed, 'right-collapsed': rightCollapsed }">
            <n-card size="small" class="left-tree-card" v-show="!leftCollapsed">
              <div class="tree-group-title">学习分组</div>
              <div
                v-for="it in leftShelfItems"
                :key="it.key"
                class="tree-item"
                :class="{ active: selectedShelf === it.key }"
                @click="selectedShelf = it.key"
              >
                <span>{{ it.label }}</span>
                <span class="tree-count">({{ it.count }})</span>
              </div>
            </n-card>

            <n-card size="small" class="middle-table-card">
              <div class="edge-toggle left" :title="leftCollapsed ? '展开左侧栏' : '收起左侧栏'" @click="leftCollapsed = !leftCollapsed">
                <Icon :icon="leftCollapsed ? chevronRightIcon : chevronLeftIcon" width="16" height="16" />
              </div>
              <div class="edge-toggle right" :title="rightCollapsed ? '展开右侧栏' : '收起右侧栏'" @click="rightCollapsed = !rightCollapsed">
                <Icon :icon="rightCollapsed ? chevronLeftIcon : chevronRightIcon" width="16" height="16" />
              </div>
              <div class="table-toolbar">
                <n-button quaternary circle size="small" title="新增记录" @click="openCreateDialog">
                  <Icon :icon="plusIcon" width="16" height="16" />
                </n-button>
                <n-button quaternary circle size="small" title="编辑记录" :disabled="!activeRow || activeRow.sourceType === 'audio'" @click="openEditDialog(activeRow)">
                  <Icon :icon="editIcon" width="16" height="16" />
                </n-button>
                <n-button quaternary circle size="small" title="编辑听力" :disabled="!activeAudio" @click="editAudio(activeAudio)">
                  <Icon :icon="settingsIcon" width="16" height="16" />
                </n-button>
                <n-button quaternary circle size="small" title="删除选中项" :disabled="!checkedRowKeys.length && !activeRow" @click="deleteSelectedRows">
                  <Icon :icon="trashIcon" width="16" height="16" />
                </n-button>
                <n-button quaternary circle size="small" title="开始复习" @click="startReviewFromSelection">
                  <Icon :icon="reviewIcon" width="16" height="16" />
                </n-button>
                <n-button quaternary circle size="small" title="刷新词汇拓展（仅单词：音标、释义、近义词、例句）" :disabled="!activeWordRecord" @click="refreshWordMeta(activeWordRecord)">
                  <Icon :icon="sparkleIcon" width="16" height="16" />
                </n-button>
                <span class="table-toolbar-hint" v-if="activeWordRecord">词汇拓展：刷新音标/释义/例句</span>
              </div>
              <template v-if="managerMode === 'random'">
                <div class="random-board">
                  <div class="random-board-head">
                    <n-tag size="small" type="success">随机抽查</n-tag>
                    <span>{{ selectedShelfLabel }}</span>
                    <span class="random-deck-index">{{ randomDecks.length ? `${randomDeckIndex + 1} / ${randomDecks.length}` : '0 / 0' }}</span>
                    <div class="random-board-actions">
                      <n-button quaternary circle size="small" title="上一组卡堆（←）" @click="gotoPrevRandomDeck">
                        <Icon :icon="chevronLeftIcon" width="16" height="16" />
                      </n-button>
                      <n-button quaternary circle size="small" title="下一组卡堆（→）" @click="gotoNextRandomDeck">
                        <Icon :icon="chevronRightIcon" width="16" height="16" />
                      </n-button>
                      <n-button quaternary circle size="small" title="翻牌（空格）" @click="toggleCurrentDeckFlip">
                        <Icon :icon="reviewIcon" width="16" height="16" />
                      </n-button>
                      <n-button quaternary circle size="small" title="抽新卡（回车）" @click="drawRandomCards">
                        <Icon :icon="shuffleIcon" width="16" height="16" />
                      </n-button>
                      <n-button quaternary circle size="small" title="返回列表模式" @click="managerMode = 'list'">
                        <Icon :icon="listIcon" width="16" height="16" />
                      </n-button>
                    </div>
                  </div>
                  <div class="random-cards-grid">
                    <div class="random-board-card" v-for="card in currentRandomCards" :key="card.id" @click="toggleRandomCardFlip(card)">
                      <div class="random-card-flip" :class="{ flipped: isCardFlipped(card) }">
                        <div class="random-card-face front" :class="{ 'random-card-face-listening-front': card.rowType === 'listening' }">
                          <div class="random-card-type">{{ rowTypeLabel(card.rowType) }}</div>
                          <template v-if="card.rowType === 'word'">
                            <div class="random-card-word">{{ card.title }}</div>
                            <div class="random-card-phonetic">{{ card.phonetic || '—' }}</div>
                            <div class="random-card-text">{{ card.sourceText || '—' }}</div>
                          </template>
                          <template v-else-if="card.rowType === 'listening'">
                            <div class="random-card-listening-main">
                              <div class="random-card-word">{{ card.title }}</div>
                              <div class="random-card-text random-card-text-clamped">{{ card.summary || card.note || '—' }}</div>
                            </div>
                            <div class="random-card-audio-row">
                              <div class="random-audio-action" :class="randomCardAudioActionState(card)" :title="randomCardAudioActionTitle(card)" @click.stop="playRandomCardAudio(card)">
                                <Icon :icon="randomCardAudioActionIcon(card)" width="24" height="24" />
                              </div>
                            </div>
                          </template>
                          <template v-else>
                            <div class="random-card-text">{{ card.sourceText || '—' }}</div>
                          </template>
                        </div>
                        <div class="random-card-face back" :class="{ 'random-card-face-listening-back': card.rowType === 'listening' }">
                          <template v-if="card.rowType === 'word'">
                            <div class="random-card-word-main random-card-listening-main-scroll">
                              <div class="random-card-head-row">
                                <div class="random-card-word">{{ card.title }}</div>
                                <div class="random-card-stars">{{ masteryStars(card.masteryLevel) }}</div>
                              </div>
                              <div class="random-card-meta-row">
                                <div class="phonetic-group" v-if="card.phoneticUk || card.phonetic">
                                  <n-tag size="small" round :bordered="false" class="phonetic-tag">
                                    <template #icon><Icon :icon="speechIcon" /></template>
                                    英 {{ card.phoneticUk || card.phonetic }}
                                  </n-tag>
                                  <n-button text size="tiny" class="phonetic-btn" @click.stop="playWordPronunciation(card.title, 'uk')">
                                    <Icon :icon="volumeUkIcon" width="14" height="14" />
                                  </n-button>
                                </div>
                                <div class="phonetic-group" v-if="card.phoneticUs || card.phonetic">
                                  <n-tag size="small" round :bordered="false" class="phonetic-tag">
                                    <template #icon><Icon :icon="speechIcon" /></template>
                                    美 {{ card.phoneticUs || card.phonetic }}
                                  </n-tag>
                                  <n-button text size="tiny" class="phonetic-btn" @click.stop="playWordPronunciation(card.title, 'us')">
                                    <Icon :icon="volumeUsIcon" width="14" height="14" />
                                  </n-button>
                                </div>
                                <n-tag v-if="card.partOfSpeech" size="small" round type="info" :bordered="false">{{ card.partOfSpeech }}</n-tag>
                              </div>
                              
                              <div class="random-card-section">
                                <div class="random-card-label">释义</div>
                                <div class="random-card-content">{{ cardWordMeanings(card).join('；') || '—' }}</div>
                              </div>

                              <div class="random-card-section" v-if="cardWordPairs(card.payload?.synonyms, card.payload?.synonyms_translations).length">
                                <div class="random-card-label">近义词</div>
                                <div class="chip-wrap">
                                  <n-tag v-for="(x, i) in cardWordPairs(card.payload?.synonyms, card.payload?.synonyms_translations)" :key="`rd-s-${card.id}-${i}`" size="small" :bordered="false">{{ x.word }}<span v-if="x.translation" class="tag-trans"> · {{ x.translation }}</span></n-tag>
                                </div>
                              </div>

                              <div class="random-card-section" v-if="cardWordPairs(card.payload?.antonyms, card.payload?.antonyms_translations).length">
                                <div class="random-card-label">反义词</div>
                                <div class="chip-wrap">
                                  <n-tag v-for="(x, i) in cardWordPairs(card.payload?.antonyms, card.payload?.antonyms_translations)" :key="`rd-a-${card.id}-${i}`" size="small" type="warning" :bordered="false">{{ x.word }}<span v-if="x.translation" class="tag-trans"> · {{ x.translation }}</span></n-tag>
                                </div>
                              </div>

                              <div class="random-card-section" v-if="cardWordPairs(card.payload?.associates, card.payload?.associates_translations).length">
                                <div class="random-card-label">联想词</div>
                                <div class="chip-wrap">
                                  <n-tag v-for="(x, i) in cardWordPairs(card.payload?.associates, card.payload?.associates_translations)" :key="`rd-r-${card.id}-${i}`" size="small" type="info" :bordered="false">{{ x.word }}<span v-if="x.translation" class="tag-trans"> · {{ x.translation }}</span></n-tag>
                                </div>
                              </div>

                              <div class="random-card-section" v-if="card.phrasePairs?.length">
                                <div class="random-card-label">常用短语</div>
                                <div class="example-line" v-for="(x, i) in card.phrasePairs" :key="`rd-p-${card.id}-${i}`">
                                  <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                                  <div class="example-target" v-if="x.target">{{ x.target }}</div>
                                </div>
                              </div>

                              <div class="random-card-section" v-if="card.examplePairs?.length">
                                <div class="random-card-label">常用例句</div>
                                <div class="example-line" v-for="(x, i) in card.examplePairs" :key="`rd-e-${card.id}-${i}`">
                                  <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                                  <div class="example-target" v-if="x.target">{{ x.target }}</div>
                                </div>
                              </div>
                            </div>
                          </template>
                          <template v-else-if="card.rowType === 'listening'">
                            <div class="random-card-listening-main random-card-listening-main-scroll" tabindex="0" @keydown.stop="handleListeningCardScrollKeydown">
                              <div class="random-card-head-row">
                                <div class="random-card-word">{{ card.title }}</div>
                              </div>
                              <div class="random-card-section">
                                <div class="random-card-label">来源 / 备注</div>
                                <div class="random-card-content">{{ card.note || '—' }}</div>
                              </div>
                              <div class="random-card-section">
                                <div class="random-card-label">原文 / 摘要</div>
                                <div class="random-card-content">{{ card.sourceText || card.summary || '—' }}</div>
                              </div>
                            </div>
                            <div class="random-card-audio-row">
                              <div class="random-audio-action" :class="randomCardAudioActionState(card)" :title="randomCardAudioActionTitle(card)" @click.stop="playRandomCardAudio(card)">
                                <Icon :icon="randomCardAudioActionIcon(card)" width="24" height="24" />
                              </div>
                            </div>
                          </template>
                          <template v-else>
                            <div class="random-card-section">
                              <div class="random-card-label">原文</div>
                              <div class="random-card-content">{{ card.sourceText || '—' }}</div>
                            </div>
                            <div class="random-card-section">
                              <div class="random-card-label">译文</div>
                              <div class="random-card-content">{{ card.translatedText || card.summary || '—' }}</div>
                            </div>
                          </template>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </template>
              <template v-else-if="selectedShelf !== 'my_listening'">
                <n-data-table
                  class="manager-data-table"
                  size="small"
                  :columns="managerColumns"
                  :data="filteredRows"
                  flex-height
                  :row-key="(row) => row.id"
                  :row-props="managerRowProps"
                  :single-line="false"
                  :checked-row-keys="checkedRowKeys"
                  @update:checked-row-keys="onManagerCheckedRowKeysChange"
                  virtual-scroll
                />
                <div class="table-footer">
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
                </div>
              </template>
              <template v-else>
                <div class="listening-toolbar">
                  <n-radio-group v-model:value="listeningKindFilter" size="small">
                    <n-radio-button value="all">全部听力</n-radio-button>
                    <n-radio-button value="remote">在线听力</n-radio-button>
                    <n-radio-button value="local">本地音频</n-radio-button>
                  </n-radio-group>
                  <n-select v-model:value="listeningProviderFilter" size="small" :options="listeningProviderOptions" style="width: 150px" />
                  <n-select
                    v-model:value="collectTopicCategory"
                    size="small"
                    :options="listeningTopicCategoryOptions"
                    clearable
                    placeholder="选择分类（支持中文）"
                    style="width: 190px"
                    @update:value="onCollectTopicCategoryChange"
                  />
                  <n-input v-model:value="collectTopic" size="small" clearable placeholder="输入主题后采集听力资源" style="width: 220px" />
                  <n-input-number v-model:value="collectLimit" size="small" :min="1" :max="50" style="width: 110px" />
                  <n-button class="listening-toolbar-circle-btn" quaternary circle size="small" title="听力源管理器" @click="sourceManagerVisible = true">
                    <Icon :icon="settingsIcon" width="16" height="16" />
                  </n-button>
                  <n-button class="listening-toolbar-circle-btn" quaternary circle size="small" title="导入本地听力" @click="importLocalAudio">
                    <Icon :icon="importIcon" width="16" height="16" />
                  </n-button>
                  <n-button class="listening-toolbar-circle-btn" quaternary circle size="small" title="按主题采集听力" :loading="collectListeningLoading || syncQueueRunning" @click="openCollectSourceDialog">
                    <Icon v-if="!collectListeningLoading && !syncQueueRunning" :icon="sparkleIcon" width="16" height="16" />
                  </n-button>
                </div>
                <n-data-table
                  class="manager-data-table"
                  size="small"
                  :columns="listeningColumns"
                  :data="pagedListeningRows"
                  flex-height
                  :scroll-x="1300"
                  :row-key="(row) => row.id"
                  :checked-row-keys="checkedRowKeys"
                  :row-props="listeningRowProps"
                  :single-line="false"
                  @update:checked-row-keys="onListeningCheckedRowKeysChange"
                />
                <div class="table-footer">
                  <n-pagination
                    size="small"
                    :page="listeningPage"
                    :page-size="listeningPageSize"
                    :item-count="listeningRows.length"
                    show-size-picker
                    :page-sizes="[20, 50, 100, 200]"
                    @update:page="onListeningPageChange"
                    @update:page-size="onListeningPageSizeChange"
                  />
                </div>
              </template>
            </n-card>

            <n-card size="small" class="right-detail-card" v-show="!rightCollapsed || isListeningImmersive" :title="selectedShelf === 'my_listening' ? '听力信息' : '词条详情 / 学习信息'">
              <div v-if="isListeningImmersive" class="immersive-global-back">
                <n-button quaternary circle size="small" title="返回听力管理界面" @click="exitListeningImmersive">
                  <Icon :icon="chevronLeftIcon" width="16" height="16" />
                </n-button>
              </div>
              <template v-if="activeWordRecord">
                <div class="word-detail-head">
                  <div class="word-detail-title">{{ activeWordRecord.word || activeWordRecord.source_text }}</div>
                  <div class="word-detail-meta">
                    <div v-if="activeWordPhoneticUk" class="phonetic-row">
                      <n-tag size="small" round>
                        <template #icon>
                          <Icon :icon="speechIcon" width="13" height="13" />
                        </template>
                        英 {{ activeWordPhoneticUk }}
                      </n-tag>
                      <n-button text size="tiny" title="英式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'uk')">
                        <Icon :icon="volumeUkIcon" width="16" height="16" />
                      </n-button>
                    </div>
                    <div v-if="activeWordPhoneticUs" class="phonetic-row">
                      <n-tag size="small" round>
                        <template #icon>
                          <Icon :icon="speechIcon" width="13" height="13" />
                        </template>
                        美 {{ activeWordPhoneticUs }}
                      </n-tag>
                      <n-button text size="tiny" title="美式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'us')">
                        <Icon :icon="volumeUsIcon" width="16" height="16" />
                      </n-button>
                    </div>
                    <n-tag v-if="activeWordRecord.part_of_speech" size="small" round type="info">
                      <template #icon>
                        <Icon :icon="badgeInfoIcon" width="13" height="13" />
                      </template>
                      词性: {{ activeWordRecord.part_of_speech }}
                    </n-tag>
                    <span>掌握度 {{ masteryStars(getMasteryLevel('record', activeWordRecord.id)) }}</span>
                  </div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">译文</div>
                  <div class="detail-content">{{ activeWordCombinedTranslation || '—' }}</div>
                </div>
                <div class="detail-block" v-if="activeWordPhrasePairs.length">
                  <div class="detail-label">常用短语</div>
                  <div class="example-line" v-for="(x, i) in activeWordPhrasePairs" :key="`wd-p-${i}`">
                    <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                    <div class="example-target" v-if="x.target">{{ x.target }}</div>
                  </div>
                </div>
                <div class="detail-block" v-if="activeWordSynonyms.length || activeWordAntonyms.length || activeWordAssociates.length">
                  <div class="detail-label">近义 / 反义 / 联想词</div>
                  <div class="detail-label tiny">近义词</div>
                  <div class="chip-wrap" v-if="activeWordSynonyms.length">
                    <n-tag v-for="(x, i) in activeWordSynonymPairs" :key="`wd-s-${i}`" size="small">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                  <div class="detail-label tiny" v-if="activeWordAntonyms.length">反义词</div>
                  <div class="chip-wrap" v-if="activeWordAntonyms.length">
                    <n-tag v-for="(x, i) in activeWordAntonymPairs" :key="`wd-a-${i}`" size="small" type="warning">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                  <div class="detail-label tiny" v-if="activeWordAssociates.length">联想词</div>
                  <div class="chip-wrap" v-if="activeWordAssociates.length">
                    <n-tag v-for="(x, i) in activeWordAssociatePairs" :key="`wd-r-${i}`" size="small" type="info">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                </div>
                <div class="detail-block" v-if="activeWordExamplePairs.length">
                  <div class="detail-label">常用例句库</div>
                  <div class="example-line" v-for="(x, i) in activeWordExamplePairs" :key="`wd-e-${i}`">
                    <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                    <div class="example-target" v-if="x.target">{{ x.target }}</div>
                  </div>
                </div>
              </template>

              <template v-else-if="activeAudio">
                <div class="listening-info-panel">
                  <div class="detail-actions">
                    <n-button type="primary" size="small" @click="enterListeningStudy">
                      <template #icon>
                        <Icon :icon="playIcon" width="14" height="14" />
                      </template>
                      进入听力学习
                    </n-button>
                  </div>
                  <div class="detail-block">
                    <div class="detail-label">标题</div>
                    <div class="detail-content">{{ activeAudio.title || '—' }}</div>
                  </div>
                  <div class="detail-block">
                    <div class="detail-label">主题 / 来源</div>
                    <div class="detail-content">{{ activeAudio.topic || '未标注' }} / {{ activeAudio.provider || '未知' }}</div>
                  </div>
                  <div class="detail-block">
                    <div class="detail-label">时长 / 创建时间</div>
                    <div class="detail-content">{{ fmtDuration(activeAudio.duration_sec) }} / {{ activeAudio.created_at || '—' }}</div>
                  </div>
                  <div class="detail-block">
                    <div class="detail-label">简明信息</div>
                    <div class="detail-content">{{ activeAudio.notes || '暂无说明' }}</div>
                  </div>
                  <div class="detail-block">
                    <div class="detail-label">同步状态</div>
                    <div class="detail-content">
                      {{ activeAudioTranscriptTimeline.length ? `已加载 ${activeAudioTranscriptTimeline.length} 段` : (activeAudioSyncGenerating ? '后台正在生成同步数据，请稍后进入学习' : '等待后台生成同步数据') }}
                    </div>
                  </div>
                <div class="detail-block">
                  <div class="detail-label">译文可用率</div>
                  <div class="detail-content">{{ formatCoverageLabel(activeAudio?.translationCoverage || { translated: 0, total: 0, ratio: 0 }) }}</div>
                </div>
                </div>
              </template>

              <template v-else-if="isListeningImmersive">
                <div class="empty-row">
                  <div class="detail-label">未找到当前听力资源</div>
                  <div class="detail-content">该条目可能已被筛选或刚被刷新，点击左上返回后重新选择。</div>
                </div>
              </template>

              <template v-else-if="activeSentenceRecord">
                <div class="detail-block">
                  <div class="detail-label">原文</div>
                  <div class="detail-content">{{ activeSentenceRecord.source_text }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">译文</div>
                  <div class="detail-content">{{ activeSentenceRecord.translated_text }}</div>
                </div>
              </template>

              <n-empty v-else description="请从中间列表选择学习项" />
            </n-card>
          </div>
        </n-tab-pane>

        <n-tab-pane name="review" tab="复习训练">
          <div class="review-layout">
            <n-card size="small" class="review-stat-card">
              <div class="review-stat-grid">
                <div class="stat-cell">
                  <div class="stat-label">总学习项</div>
                  <div class="stat-value">{{ totalLearningItemCount }}</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">当前内容掌握度</div>
                  <div class="stat-value">{{ currentReviewMasteryLabel }}</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">掌握占比</div>
                  <div class="stat-value">{{ masteryPercent }}%</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">当前卡组</div>
                  <div class="stat-value">{{ reviewRows.length ? `${reviewIndex + 1}/${reviewRows.length}` : '0/0' }}</div>
                </div>
              </div>
              <div class="mastery-bar-track">
                <div class="mastery-bar-fill" :style="{ width: `${masteryPercent}%` }"></div>
              </div>
            </n-card>

            <n-card size="small" class="review-card-panel">
              <div class="review-topbar">
                <n-radio-group v-model:value="reviewType" size="small">
                  <n-radio-button value="mixed">混合</n-radio-button>
                  <n-radio-button value="word">单词</n-radio-button>
                  <n-radio-button value="sentence">例句</n-radio-button>
                  <n-radio-button value="paragraph">段落</n-radio-button>
                  <n-radio-button value="listening">听力</n-radio-button>
                </n-radio-group>
                <n-button size="small" @click="shuffleReviewRows">洗牌</n-button>
              </div>

              <div v-if="currentReviewItem" class="review-card">
                <div class="review-card-type">
                  <n-tag size="small" type="info">{{ rowTypeLabel(currentReviewItem.rowType) }}</n-tag>
                  <span>{{ reviewIndex + 1 }} / {{ reviewRows.length }}</span>
                </div>
                <div class="review-card-front">
                  <template v-if="currentReviewItem.rowType === 'word'">
                    <div class="review-word">{{ currentReviewItem.title }}</div>
                    <div class="review-phonetic">{{ currentReviewItem.phonetic || '—' }}</div>
                  </template>
                  <template v-else-if="currentReviewItem.rowType === 'listening'">
                    <div class="review-listening-player-wrap">
                      <audio
                        ref="reviewAudioPlayerRef"
                        class="review-listening-player"
                        controls
                        :src="reviewAudioPlayableSrc"
                        @timeupdate="handleReviewAudioTimeUpdate"
                        @loadedmetadata="handleReviewAudioLoadedMetadata"
                        @canplay="handleListeningMediaCanPlay('review', $event)"
                        @play="handleListeningMediaPlay('review', $event)"
                        @error="handleListeningMediaError('review', $event)"
                      ></audio>
                      <div class="review-phonetic">点击显示答案后可查看完整原文</div>
                    </div>
                  </template>
                  <template v-else>
                    <div class="review-text">{{ currentReviewItem.sourceText }}</div>
                  </template>
                </div>
                <div class="review-card-answer" v-if="reviewShowAnswer">
                  <div class="review-answer-title">答案</div>
                  <template v-if="currentReviewItem.rowType === 'word'">
                    <div class="review-answer-meta">
                      <div class="review-answer-chip">音标：{{ currentReviewItem.phonetic || '—' }}</div>
                      <div class="review-answer-chip">词性：{{ currentReviewItem.partOfSpeech || '—' }}</div>
                    </div>
                    <div class="review-answer-text">{{ currentReviewItem.combinedTranslation || currentReviewItem.translatedText || '—' }}</div>
                    <div v-if="currentReviewItem.examplePairs?.length" class="review-examples">
                      <div class="review-answer-title">例句</div>
                      <div class="review-example-item" v-for="(x, i) in currentReviewItem.examplePairs" :key="`rv-ex-${i}`">
                        <div class="review-answer-text">{{ i + 1 }}. {{ x.source }}</div>
                        <div class="review-answer-text review-answer-target" v-if="x.target">{{ x.target }}</div>
                      </div>
                    </div>
                  </template>
                  <template v-else-if="currentReviewItem.rowType === 'listening'">
                    <div class="review-answer-title-row">
                      <div class="review-answer-title">原文与译文</div>
                      <n-button quaternary circle size="small" class="transcript-mode-btn" :title="`切换显示模式（当前：${transcriptDisplayModeLabel}）`" @click="cycleTranscriptDisplayMode">
                        <Icon :icon="languagesIcon" width="13" height="13" />
                      </n-button>
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
                        <div class="transcript-source" v-if="transcriptDisplayMode !== 'target'">{{ item.line.text }}</div>
                        <div class="transcript-target" v-if="transcriptDisplayMode !== 'source'">{{ resolveTranscriptTargetText(item.line) }}</div>
                      </div>
                    </div>
                    <div v-else class="review-answer-text">同步数据尚未就绪，请稍后重试。</div>
                  </template>
                  <template v-else>
                    <div class="review-answer-text">{{ currentReviewItem.summary || currentReviewItem.translatedText || '—' }}</div>
                    <div v-if="currentReviewItem.examples?.length" class="review-examples">
                      <div class="review-answer-title">例句</div>
                      <div class="review-answer-text" v-for="(x, i) in currentReviewItem.examples" :key="`rv-ex-${i}`">{{ x }}</div>
                    </div>
                  </template>
                </div>
                <div class="review-actions">
                  <n-button size="small" @click="reviewShowAnswer = !reviewShowAnswer">{{ reviewShowAnswer ? '隐藏答案' : '显示答案' }}</n-button>
                  <n-button size="small" type="success" @click="gradeAndNext('known')">认识</n-button>
                  <n-button size="small" type="warning" @click="gradeAndNext('fuzzy')">模糊</n-button>
                  <n-button size="small" type="error" @click="gradeAndNext('unknown')">不认识</n-button>
                  <n-button size="small" @click="gotoPrevReview">上一个</n-button>
                  <n-button size="small" @click="gotoNextReview">下一个</n-button>
                </div>
              </div>
              <n-empty v-else description="当前筛选下暂无可复习内容" />
            </n-card>

            <n-card size="small" class="audio-form-card" title="复习筛选">
              <n-form :model="reviewFilter" label-placement="left" label-width="90">
                <n-form-item label="添加时间">
                  <n-select v-model:value="reviewFilter.createdPeriod" :options="createdPeriodOptions" />
                </n-form-item>
                <n-form-item label="上次复习">
                  <n-select v-model:value="reviewFilter.lastReviewPeriod" :options="lastReviewPeriodOptions" />
                </n-form-item>
                <n-form-item label="掌握程度">
                  <n-select v-model:value="reviewFilter.masteryBand" :options="masteryBandOptions" />
                </n-form-item>
                <n-form-item label="排序方式">
                  <n-select v-model:value="reviewFilter.orderBy" :options="reviewOrderOptions" />
                </n-form-item>
              </n-form>
              <n-space>
                <n-button type="primary" @click="startReviewFromSelection">应用筛选并开始</n-button>
                <n-button @click="resetReviewFilter">重置筛选</n-button>
              </n-space>
            </n-card>
          </div>
        </n-tab-pane>
      </n-tabs>

      <n-modal v-model:show="audioManagerVisible" preset="card" style="width: 760px" title="听力资源管理">
        <n-form :model="audioForm" label-placement="left" label-width="72">
          <n-form-item label="标题">
            <n-input v-model:value="audioForm.title" placeholder="例如：BBC-News-01" />
          </n-form-item>
          <n-form-item label="类型">
            <n-select v-model:value="audioForm.source_kind" :options="audioKindOptions" />
          </n-form-item>
          <n-form-item label="地址">
            <n-input v-model:value="audioForm.source_uri" placeholder="本地路径或在线 URL" />
          </n-form-item>
          <n-form-item label="时长">
            <n-input-number v-model:value="audioForm.duration_sec" :min="0" />
          </n-form-item>
          <n-form-item label="转写">
            <n-input v-model:value="audioForm.transcript" type="textarea" :autosize="{ minRows: 2, maxRows: 5 }" />
          </n-form-item>
          <n-form-item label="备注">
            <n-input v-model:value="audioForm.notes" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
          </n-form-item>
          <n-form-item label="主题标签">
            <n-input v-model:value="audioForm.topic" placeholder="例如：旅行、科技、商务英语" />
          </n-form-item>
          <n-form-item label="资源来源">
            <n-input v-model:value="audioForm.provider" placeholder="例如：archive / wikimedia / local" />
          </n-form-item>
        </n-form>
        <template #action>
          <n-space justify="end">
            <n-button @click="importLocalAudio">导入本地音频</n-button>
            <n-button @click="openCollectSourceDialog">按主题采集</n-button>
            <n-button type="primary" @click="saveAudio">保存资源</n-button>
          </n-space>
        </template>
      </n-modal>

      <n-modal v-model:show="collectSourceDialogVisible" preset="card" style="width: 720px" title="本次抓取来源">
        <n-space vertical :size="12">
          <n-radio-group v-model:value="collectSourceMode" size="small">
            <n-radio-button value="default">默认策略（平均从每个源抓取）</n-radio-button>
            <n-radio-button value="custom">自定义本次来源</n-radio-button>
          </n-radio-group>
          <n-tag size="small" :type="collectAvailableSourceOptions.length ? 'info' : 'warning'">可用来源 {{ collectAvailableSourceOptions.length }} 个</n-tag>
          <div v-if="!collectAvailableSourceOptions.length" class="collect-source-hint">{{ collectSourceEmptyHint }}</div>
          <n-select
            v-if="collectSourceMode === 'custom'"
            v-model:value="collectSourceIds"
            multiple
            clearable
            filterable
            :options="collectAvailableSourceOptions"
            placeholder="选择本次抓取来源（可多选）"
          />
        </n-space>
        <template #action>
          <n-space justify="end">
            <n-button @click="collectSourceDialogVisible = false">取消</n-button>
            <n-button type="primary" :loading="collectListeningLoading" @click="confirmCollectListeningByTopic">开始抓取</n-button>
          </n-space>
        </template>
      </n-modal>

      <n-modal v-model:show="sourceManagerVisible" preset="card" style="width: 860px" title="听力源管理器">
        <div class="source-manager-head">
          <n-input v-model:value="newSourceName" size="small" placeholder="来源名称（如 TED 技术演讲）" style="width: 180px" />
          <n-input v-model:value="newSourceProvider" size="small" placeholder="来源标识（如 ted）" style="width: 120px" />
          <n-input v-model:value="newSourceFeedUrl" size="small" placeholder="RSS地址，可用 {topic} 占位符" style="flex: 1" />
          <n-select v-model:value="newSourceLanguage" size="small" style="width: 110px" :options="learningLangOptions" />
          <n-input-number v-model:value="newSourceMaxDurationMin" size="small" :min="1" :max="30" style="width: 92px" />
          <n-button size="small" type="primary" @click="addListeningSource">新增</n-button>
        </div>
        <div class="source-manager-list">
          <div class="source-item" v-for="src in listeningSources" :key="src.id">
            <n-switch v-model:value="src.enabled" @update:value="persistListeningSources" />
            <n-input v-model:value="src.name" size="small" style="width: 170px" @blur="persistListeningSources" />
            <n-input v-model:value="src.provider" size="small" style="width: 100px" @blur="persistListeningSources" />
            <n-input v-model:value="src.feed_url" size="small" style="flex: 1" @blur="persistListeningSources" />
            <n-select v-model:value="src.language_tag" size="small" style="width: 110px" :options="learningLangOptions" @update:value="persistListeningSources" />
            <n-input-number v-model:value="src.max_duration_min" size="small" :min="1" :max="30" style="width: 92px" @update:value="persistListeningSources" />
            <n-button quaternary circle size="small" title="删除来源" @click="removeListeningSource(src.id)">
              <Icon :icon="trashIcon" width="16" height="16" />
            </n-button>
          </div>
        </div>
      </n-modal>

      <n-modal v-model:show="recordEditVisible" preset="card" style="width: 680px" :title="recordEditMode === 'edit' ? '编辑学习记录' : '新增学习记录'">
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
          <n-form-item label="来源语言">
            <n-input v-model:value="recordForm.source_lang" />
          </n-form-item>
          <n-form-item label="目标语言">
            <n-input v-model:value="recordForm.target_lang" />
          </n-form-item>
          <n-form-item label="引擎">
            <n-input v-model:value="recordForm.engine" />
          </n-form-item>
        </n-form>
        <template #action>
          <n-space justify="end">
            <n-button @click="recordEditVisible = false">取消</n-button>
            <n-button type="primary" @click="saveRecord">保存</n-button>
          </n-space>
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
  NDataTable,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NMessageProvider,
  NModal,
  NPagination,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpace,
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
const collectTopicCategory = ref('')
const collectTopic = ref('')
const collectLimit = ref(15)
const collectListeningLoading = ref(false)
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
const masteryStore = ref({ record: {}, audio: {} })
const shelfCounts = reactive({ all: 0, my_vocab: 0, my_sentences: 0, my_paragraphs: 0, my_listening: 0 })
const audioManagerVisible = ref(false)
const randomDecks = ref([])
const randomDeckIndex = ref(0)
const randomFlippedMap = ref({})
const randomCardAudioPlayingId = ref('')
const randomCardAudioPaused = ref(false)
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

const reviewFilter = reactive({
  createdPeriod: 'all',
  lastReviewPeriod: 'all',
  masteryBand: 'all',
  orderBy: 'created_desc',
})

const recordEditVisible = ref(false)
const recordEditMode = ref('create')
const recordForm = reactive({
  id: '',
  source_text: '',
  translated_text: '',
  record_type: 'sentence',
  source_lang: 'auto',
  target_lang: 'zh',
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
const learningLangLabel = computed(() => resolveLearningLangLabel(learningLang.value))
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
    return { level: 0, reviewed: 0, mastered: 0, lastResult: '', lastReviewedAt: '' }
  }
  return {
    level: toNum(row.level),
    reviewed: toNum(row.reviewed),
    mastered: toNum(row.mastered),
    lastResult: String(row.lastResult || ''),
    lastReviewedAt: String(row.lastReviewedAt || ''),
  }
}

const upsertMasteryEntry = (sourceType, rawId, result) => {
  const key = String(rawId || '')
  if (!key) return
  const current = getMasteryEntry(sourceType, key)
  let level = current.level
  if (result === 'known') level += 1
  if (result === 'fuzzy') level += 0
  if (result === 'unknown') level -= 1
  level = Math.min(5, Math.max(0, level))

  const next = {
    level,
    reviewed: current.reviewed + 1,
    mastered: current.mastered + (result === 'known' ? 1 : 0),
    lastResult: result,
    lastReviewedAt: new Date().toISOString(),
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

const pagedListeningRows = computed(() => {
  const pageSize = Math.max(1, Number(listeningPageSize.value || 100))
  const page = Math.max(1, Number(listeningPage.value || 1))
  const offset = (page - 1) * pageSize
  return listeningRows.value.slice(offset, offset + pageSize)
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

const enterListeningStudy = async () => {
  if (!activeRow.value || activeRow.value.sourceType !== 'audio') {
    message.warning('请先选择一个听力条目')
    return
  }
  const audioId = String(activeRow.value.rawId || '').trim()
  if (audioId) {
    activeAudioSyncGenerating.value = true
    try {
      const refreshed = await invoke('language_learning_backfill_sync_translation', {
        audioId,
        learningLang: learningLang.value,
      })
      if (refreshed?.id) {
        const idx = audioResources.value.findIndex((x) => String(x?.id || '') === String(refreshed.id))
        if (idx >= 0) {
          audioResources.value[idx] = refreshed
        }
      }
      const latestSyncPath = String(refreshed?.sync_json_uri || activeAudio.value?.sync_json_uri || '').trim()
      const timeline = await loadSyncTimelineByJsonPath(latestSyncPath)
      if (Array.isArray(timeline) && timeline.length) {
        activeAudioSyncTimeline.value = timeline
        audioTranslationCoverageMap.value = {
          ...audioTranslationCoverageMap.value,
          [audioId]: resolveTranslationCoverageFromTimeline(timeline),
        }
      }
    } catch {
      activeAudioSyncGenerating.value = triggerSyncGenerationForAudioId(audioId, false, learningLang.value)
      if (shouldRunQuickTimelineTranslate(audioId)) {
        const patchedTimeline = await quickTranslateTimelineToZh(activeAudioTranscriptTimeline.value, activeTimelineQuickTranslating)
        if (Array.isArray(patchedTimeline) && patchedTimeline.length) {
          activeAudioSyncTimeline.value = patchedTimeline
        }
      }
    } finally {
      if (!timelineHasMissingTranslation(activeAudioTranscriptTimeline.value)) {
        activeAudioSyncGenerating.value = false
      }
    }
  }
  openListeningImmersive(activeRow.value)
}

const exitListeningImmersive = () => {
  listeningImmersiveMode.value = false
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

const reviewRowsByType = computed(() => {
  const rows = filteredRows.value.filter((row) => {
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
    return true
  })
  if (reviewType.value === 'mixed') return rows
  return rows.filter((r) => r.rowType === reviewType.value)
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

const gotoNextReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value + 1) % reviewRows.value.length
  reviewShowAnswer.value = false
}

const gotoPrevReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value - 1 + reviewRows.value.length) % reviewRows.value.length
  reviewShowAnswer.value = false
}

const gradeAndNext = (result) => {
  const cur = currentReviewItem.value
  if (!cur) return
  upsertMasteryEntry(cur.sourceType, cur.rawId, result)
  gotoNextReview()
}

const startReviewFromSelection = () => {
  activeTab.value = 'review'
  rebuildReviewRows()
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
        message.error(`听力转写失败: ${title}，请检查 whisper/翻译配置后重试`)
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
  syncQueuedTasks.push({ id, learningLang: taskLearningLang })
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
    list.forEach((id) => triggerSyncGenerationForAudioId(id, false, learningLang.value))
  } catch {}
}

const refreshAll = async () => {
  await refreshRecords()
  await refreshAudio()
  await refreshShelfCounts()
  checkedRowKeys.value = []
  if (!activeRow.value && filteredRows.value.length) {
    activeRow.value = filteredRows.value[0]
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
      targetLang: row.target_lang || 'zh',
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
    message.success('词汇拓展已更新')
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

const removeAudio = async (row) => {
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
}

const openCreateDialog = () => {
  recordEditMode.value = 'create'
  recordForm.id = ''
  recordForm.source_text = ''
  recordForm.translated_text = ''
  recordForm.record_type = selectedShelf.value === 'my_paragraphs' ? 'paragraph' : 'sentence'
  recordForm.source_lang = 'auto'
  recordForm.target_lang = 'zh'
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
  recordForm.target_lang = String(row.payload?.target_lang || learningLang.value)
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

const deleteSelectedRows = async () => {
  const selected = allRows.value.filter((r) => checkedRowKeys.value.includes(r.id))
  const fallback = activeRow.value ? [activeRow.value] : []
  const rows = selected.length ? selected : fallback
  if (!rows.length) {
    message.warning('请先选择要删除的项目')
    return
  }
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
}

const openCollectSourceDialog = () => {
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
  collectSourceDialogVisible.value = false
  await collectListeningByTopic(selected.map(({ id, ...rest }) => rest))
}

const collectListeningByTopic = async (selectedSourcesOverride = null) => {
  collectListeningLoading.value = true
  try {
    const baseTopic = String(collectTopic.value || audioForm.topic || '').trim()
    const categoryLabel = resolveTopicCategoryLabel(collectTopicCategory.value)
    const topic = (baseTopic || categoryLabel).trim()
    if (!topic) {
      message.warning('请先输入听力主题或选择分类')
      return
    }
    const limit = Number(collectLimit.value || 15)
    const selectedSources = Array.isArray(selectedSourcesOverride) && selectedSourcesOverride.length
      ? selectedSourcesOverride
      : collectAvailableSourcePayload.value.map(({ id, ...rest }) => rest)
    if (!selectedSources.length) {
      message.warning(collectSourceEmptyHint.value)
      return
    }
    const rows = await invoke('language_learning_collect_audio_by_topic', {
      input: {
        topic,
        learning_lang: learningLang.value,
        limit,
        sources: selectedSources,
      }
    })
    if (Array.isArray(rows) && rows.length) {
      message.success(`已采集 ${rows.length} 条听力资源`)
      rows.forEach((item) => {
        if (item?.id) triggerSyncGenerationForAudioId(item.id, false)
      })
    } else {
      message.warning('未采集到可用资源，请更换主题重试')
    }
    await refreshAudio()
    await refreshShelfCounts()
    collectTopic.value = baseTopic || categoryLabel
    audioForm.topic = collectTopic.value
  } catch (err) {
    message.error(`主题采集失败: ${String(err?.message || err || '')}`)
  } finally {
    collectListeningLoading.value = false
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
  const candidates = filteredRows.value
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

const playRandomCardAudio = (card) => {
  if (String(card?.rowType || '') !== 'listening') return
  const cardId = String(card?.id || '')
  if (!cardId || !sanitizeMediaUri(card?.payload?.source_uri || '')) return
  if (randomCardAudioPlayer && randomCardAudioPlayingId.value === cardId) {
    if (!randomCardAudioPlayer.paused) {
      randomCardAudioPlayer.pause()
      randomCardAudioPaused.value = true
      return
    }
    randomCardAudioPlayer.currentTime = 0
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
    randomCardAudioPlayer.onended = null
    randomCardAudioPlayer.onerror = null
    randomCardAudioPlayer.pause()
    randomCardAudioPlayer = null
  }
  randomCardAudioPlayingId.value = ''
  randomCardAudioPaused.value = false
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
      to: 'zh',
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
  if (!row) return
  triggerSyncGenerationForAudioId(row.id, true, 'zh')
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
  activeAudioSyncTimeline.value = await loadSyncTimelineByJsonPath(activeAudio.value?.sync_json_uri || '')
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
    activeAudioSyncGenerating.value = triggerSyncGenerationForAudioId(activeAudio.value.id, false, learningLang.value)
    if (shouldRunQuickTimelineTranslate(activeAudio.value.id)) {
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
  if (activeAudio.value?.id && !activeAudio.value?.sync_json_uri && !activeAudioSyncTimeline.value.length) {
    triggerSyncGenerationForAudioId(activeAudio.value.id, false, 'zh')
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
  reviewAudioSyncTimeline.value = await loadSyncTimelineByJsonPath(row?.payload?.sync_json_uri || '')
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
    triggerSyncGenerationForAudioId(row.payload.id, false, learningLang.value)
    if (shouldRunQuickTimelineTranslate(row.payload.id)) {
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

watch([reviewType, filteredRows, () => reviewFilter.createdPeriod, () => reviewFilter.lastReviewPeriod, () => reviewFilter.masteryBand, () => reviewFilter.orderBy], () => {
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
  loadMasteryStore()
  loadListeningSources()
  persistListeningSources()
  await cleanupDuplicateListeningTopics()
  await refreshAll()
  await resumePendingSyncTasks()
  if (filteredRows.value.length) {
    activeRow.value = filteredRows.value[0]
  }
  rebuildReviewRows()
})

onBeforeUnmount(() => {
  window.removeEventListener('error', handleWindowResizeObserverWarning, true)
  window.removeEventListener('keydown', handleRandomBoardKeydown)
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
})
</script>

<style scoped>
.lang-learn-page {
  height: 100vh;
  box-sizing: border-box;
  padding: 12px;
  background: var(--guaxi-app-bg);
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
}

.manager-header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.title-main {
  font-size: 22px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.toolbar-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.page-tabs {
  flex: 1 1 auto;
  min-height: 0;
}

.listening-immersive-page {
  height: calc(100vh - 150px);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: var(--guaxi-panel-bg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.manager-layout {
  height: calc(100vh - 150px);
  display: grid;
  grid-template-columns: 260px 1fr 420px;
  gap: 10px;
}

.manager-layout.left-collapsed {
  grid-template-columns: 1fr 420px;
}

.manager-layout.right-collapsed {
  grid-template-columns: 260px 1fr;
}

.manager-layout.left-collapsed.right-collapsed {
  grid-template-columns: 1fr;
}

.manager-layout.listening-immersive {
  grid-template-columns: 0 0 1fr;
}

.manager-layout.listening-immersive .left-tree-card,
.manager-layout.listening-immersive .middle-table-card {
  display: none;
}

.left-tree-card,
.middle-table-card,
.right-detail-card {
  min-height: 0;
  overflow: hidden;
}

.left-tree-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: auto;
}

.tree-group-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 4px;
}

.tree-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 82%, transparent);
}

.tree-item.active {
  border-color: var(--primary-color, #4acadb);
  background: color-mix(in srgb, var(--primary-color, #4acadb) 14%, var(--guaxi-panel-bg));
}

.tree-count {
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.middle-table-card {
  display: flex;
  flex-direction: column;
  position: relative;
}

.edge-toggle {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 22px;
  height: 46px;
  border-radius: 11px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
  cursor: pointer;
  z-index: 6;
  transition: all .2s ease;
}

.edge-toggle.left {
  left: -12px;
}

.edge-toggle.right {
  right: -12px;
}

.edge-toggle:hover {
  border-color: var(--primary-color, #4acadb);
  color: var(--primary-color, #4acadb);
  transform: translateY(-50%) scale(1.04);
}

.table-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 8px;
  align-items: center;
}

.table-toolbar-hint {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px dashed color-mix(in srgb, var(--primary-color, #4acadb) 34%, var(--guaxi-border));
  background: color-mix(in srgb, var(--primary-color, #4acadb) 8%, var(--guaxi-panel-bg));
}

.table-footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

.random-board {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 640px;
}

.random-board-head {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--guaxi-text-secondary);
}

.random-deck-index {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
}

.random-board-actions {
  margin-left: auto;
  display: flex;
  gap: 6px;
}

.random-cards-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.random-board-card {
  min-height: 520px;
  perspective: 1400px;
  animation: cardPop .22s ease;
}

.random-card-flip {
  position: relative;
  width: 100%;
  min-height: 520px;
  transform-style: preserve-3d;
  transition: transform .38s ease;
}

.random-card-flip.flipped {
  transform: rotateY(180deg);
}

.random-card-face {
  position: absolute;
  inset: 0;
  border: 1px solid var(--guaxi-border);
  border-radius: 14px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  backface-visibility: hidden;
  background: linear-gradient(155deg, color-mix(in srgb, var(--guaxi-panel-bg) 93%, #8dd2ff 7%), color-mix(in srgb, var(--guaxi-panel-bg) 93%, #89f0cb 7%));
  overflow: auto;
}

.random-card-face.back {
  transform: rotateY(180deg);
  background: linear-gradient(155deg, color-mix(in srgb, var(--guaxi-panel-bg) 93%, #b7b8ff 7%), color-mix(in srgb, var(--guaxi-panel-bg) 93%, #8df0d5 7%));
}

.random-card-face-listening-front {
  overflow: hidden;
}

.random-card-face-listening-back {
  overflow: hidden;
}

.random-card-type {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.random-card-word {
  font-size: 24px;
  font-weight: 700;
  line-height: 1.2;
}

.random-card-phonetic {
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.random-card-text {
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 13px;
}

.random-card-listening-main {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: hidden;
}

.random-card-word-main {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.random-card-listening-main-scroll {
  overflow-y: auto;
  padding-right: 2px;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--primary-color, #4acadb) 32%, transparent) transparent;
}

.random-card-listening-main-scroll:focus-visible {
  outline: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 42%, transparent);
  outline-offset: 2px;
  border-radius: 8px;
}

.random-card-listening-main-scroll::-webkit-scrollbar {
  width: 6px;
}

.random-card-listening-main-scroll::-webkit-scrollbar-track {
  background: transparent;
}

.random-card-listening-main-scroll::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: color-mix(in srgb, var(--primary-color, #4acadb) 28%, transparent);
}

.random-card-listening-main-scroll::-webkit-scrollbar-thumb:hover {
  background: color-mix(in srgb, var(--primary-color, #4acadb) 42%, transparent);
}

.random-card-text-clamped {
  overflow: hidden;
}

.random-card-audio-row {
  margin-top: auto;
  min-height: 58px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.random-card-subtitle {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.random-card-list {
  border-top: 1px dashed var(--guaxi-border);
  padding-top: 8px;
}

.random-card-answer {
  border-top: 1px dashed var(--guaxi-border);
  padding-top: 8px;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 70%, var(--guaxi-text-primary));
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.random-audio-action {
  margin-inline: auto;
  width: 54px;
  height: 54px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  color: var(--primary-color, #4acadb);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 84%, transparent);
  transition: transform .2s ease, border-color .2s ease, box-shadow .2s ease, background-color .2s ease;
}

.random-audio-action.idle {
  color: color-mix(in srgb, var(--guaxi-text-secondary) 78%, var(--primary-color, #4acadb));
  border-color: color-mix(in srgb, var(--guaxi-border) 88%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.random-audio-action.paused {
  color: color-mix(in srgb, #f59f00 78%, var(--guaxi-text-primary));
  border-color: color-mix(in srgb, #f59f00 42%, var(--guaxi-border));
  background: color-mix(in srgb, #f59f00 12%, var(--guaxi-panel-bg));
}

.random-audio-action:hover {
  transform: scale(1.06);
  border-color: var(--primary-color, #4acadb);
}

.random-audio-action.playing {
  color: var(--primary-color, #4acadb);
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 80%, var(--guaxi-border));
  background: color-mix(in srgb, var(--primary-color, #4acadb) 14%, var(--guaxi-panel-bg));
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary-color, #4acadb) 22%, transparent);
  animation: randomAudioPulse 1.25s ease-in-out infinite;
}

@keyframes randomAudioPulse {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.08);
  }
  100% {
    transform: scale(1);
  }
}

.listening-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 8px;
  align-items: center;
}

.listening-toolbar-circle-btn {
  width: 28px;
  min-width: 28px;
  height: 28px;
  padding: 0;
}

.listening-toolbar-circle-btn :deep(.n-button__content) {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.listening-toolbar-circle-btn :deep(.n-button__icon) {
  margin-right: 0 !important;
}

.listening-toolbar-circle-btn :deep(.n-base-loading) {
  margin-right: 0 !important;
}

.listening-info-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.middle-table-card :deep(.n-card__content) {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.manager-data-table {
  flex: 1 1 auto;
  min-height: 0;
}

.middle-table-card :deep(.n-data-table-tbody .n-data-table-tr) {
  cursor: pointer;
}

.middle-table-card :deep(.n-data-table-tbody .n-data-table-tr:hover) {
  background: color-mix(in srgb, var(--primary-color, #4acadb) 10%, var(--guaxi-panel-bg));
}

.list-item-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.list-item-title {
  font-weight: 600;
}

.list-item-sub {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.topic-cell {
  max-width: 100%;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 34%, var(--guaxi-border));
  background: color-mix(in srgb, var(--primary-color, #4acadb) 8%, var(--guaxi-panel-bg));
  color: color-mix(in srgb, var(--primary-color, #4acadb) 78%, var(--guaxi-text-primary));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mastery-stars {
  font-size: 12px;
  color: #f5a524;
  letter-spacing: 1px;
}

.right-detail-card {
  overflow: auto;
}

.immersive-global-back {
  display: flex;
  align-items: center;
  gap: 10px;
  justify-content: flex-start;
  margin-bottom: 12px;
}

.immersive-back-btn {
  width: 34px;
  height: 34px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 46%, var(--guaxi-border));
  background: linear-gradient(135deg, color-mix(in srgb, var(--primary-color, #4acadb) 22%, var(--guaxi-panel-bg)), var(--guaxi-panel-bg));
  box-shadow: 0 8px 18px color-mix(in srgb, var(--primary-color, #4acadb) 24%, transparent);
  transition: transform .18s ease, box-shadow .18s ease, border-color .18s ease, background-color .18s ease;
}

.immersive-back-btn:hover {
  transform: translateY(-1px) scale(1.06);
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 78%, var(--guaxi-border));
  box-shadow: 0 10px 22px color-mix(in srgb, var(--primary-color, #4acadb) 34%, transparent);
}

.immersive-back-btn:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--primary-color, #4acadb) 62%, transparent);
  outline-offset: 2px;
}

.immersive-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
  padding: 4px 12px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 30%, var(--guaxi-border));
  background: color-mix(in srgb, var(--primary-color, #4acadb) 10%, var(--guaxi-header-bg));
}

.listening-detail-layout {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.listening-detail-layout.immersive {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
}

.listening-player-pane,
.listening-transcript-pane {
  min-height: 0;
  height: 100%;
}

.listening-player-pane {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.immersive-media-shell {
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 24%, var(--guaxi-border));
  border-radius: 16px;
  padding: 8px;
  background: linear-gradient(155deg, color-mix(in srgb, var(--primary-color, #4acadb) 10%, var(--guaxi-panel-bg)), var(--guaxi-panel-bg));
}

.listening-video-player {
  min-height: 260px;
  background: #0f1721;
  object-fit: contain;
}

.listening-player-topbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.listening-player {
  width: 100%;
  margin-top: 8px;
  margin-bottom: 12px;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 26%, var(--guaxi-border));
  background: linear-gradient(135deg, color-mix(in srgb, var(--primary-color, #4acadb) 10%, var(--guaxi-panel-bg)), var(--guaxi-panel-bg));
  box-shadow: 0 8px 24px color-mix(in srgb, var(--primary-color, #4acadb) 18%, transparent);
  accent-color: var(--primary-color, #4acadb);
}

.listening-player::-webkit-media-controls-panel {
  background: transparent;
}

.listening-player::-webkit-media-controls-play-button,
.listening-player::-webkit-media-controls-mute-button {
  filter: saturate(1.15) contrast(1.08);
}

.playback-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, var(--primary-color, #4acadb));
}

.transcript-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.transcript-head-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.transcript-mode-btn {
  width: 32px;
  height: 32px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 46%, var(--guaxi-border));
  color: var(--primary-color, #4acadb);
  background: color-mix(in srgb, var(--primary-color, #4acadb) 12%, var(--guaxi-panel-bg));
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary-color, #4acadb) 24%, transparent);
}

.transcript-font-btn {
  width: 32px;
  height: 32px;
  font-size: 11px;
  font-weight: 700;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 40%, var(--guaxi-border));
  background: color-mix(in srgb, var(--primary-color, #4acadb) 10%, var(--guaxi-panel-bg));
  color: color-mix(in srgb, var(--primary-color, #4acadb) 86%, var(--guaxi-text-primary));
}

.listening-transcript-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.transcript-list {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 8px 10px;
  flex: 1 1 auto;
  min-height: 0;
  max-height: none;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--guaxi-panel-bg);
}

.transcript-line {
  display: block;
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
  line-height: 1.72;
  padding: 7px 10px;
  border-radius: 6px;
  border: 1px solid transparent;
  color: var(--guaxi-text-secondary);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  cursor: pointer;
  transform-origin: center left;
  transition: background-color 0.18s ease, border-color 0.18s ease, transform 0.2s ease, box-shadow 0.2s ease, color 0.18s ease;
}

.transcript-line.hovered {
  background: color-mix(in srgb, var(--primary-color, #4acadb) 8%, var(--guaxi-panel-bg));
}

.transcript-line.active {
  background: color-mix(in srgb, var(--primary-color, #4acadb) 20%, var(--guaxi-panel-bg));
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 62%, var(--guaxi-border));
  color: var(--guaxi-text-primary);
  transform: scale(1.018);
  box-shadow: 0 10px 24px color-mix(in srgb, var(--primary-color, #4acadb) 32%, transparent);
  border-left: 4px solid var(--primary-color, #4acadb);
  padding-left: 12px;
}

.transcript-line.active .transcript-source {
  color: var(--guaxi-text-primary);
  font-weight: 700;
  text-shadow: 0 0 12px color-mix(in srgb, var(--primary-color, #4acadb) 38%, transparent);
}

.transcript-line.active .transcript-target {
  color: color-mix(in srgb, var(--primary-color, #4acadb) 82%, var(--guaxi-text-primary));
}

.transcript-source {
  font-size: calc(16px * var(--transcript-font-scale, 1));
  line-height: 1.65;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.transcript-target {
  margin-top: 3px;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 72%, var(--guaxi-text-primary));
  font-size: calc(15px * var(--transcript-font-scale, 1));
  line-height: 1.62;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.word-detail-head {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px 12px;
  margin-bottom: 10px;
  background: color-mix(in srgb, var(--guaxi-header-bg) 70%, transparent);
}

.word-detail-title {
  font-size: 28px;
  font-weight: 700;
  line-height: 1.05;
}

.word-detail-meta {
  margin-top: 8px;
  color: var(--guaxi-text-secondary);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.phonetic-row {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.detail-block {
  margin-bottom: 12px;
}

.detail-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.detail-label.tiny {
  font-size: 12px;
  opacity: 0.85;
  margin-top: 6px;
}

.detail-content {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 8px 10px;
  line-height: 1.56;
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--guaxi-panel-bg);
}

.source-manager-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.source-manager-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 420px;
  overflow: auto;
}

.source-item {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 8px;
}

.collect-source-hint {
  font-size: 12px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
  padding: 6px 10px;
  border-radius: 8px;
  border: 1px dashed color-mix(in srgb, #f0a020 45%, var(--guaxi-border));
  background: color-mix(in srgb, #f0a020 12%, var(--guaxi-panel-bg));
}

.chip-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.example-line {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 8px 10px;
  margin-bottom: 6px;
  line-height: 1.5;
}

.example-src {
  line-height: 1.55;
}

.example-target {
  margin-top: 4px;
  line-height: 1.55;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 70%, var(--guaxi-text-primary));
}

.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 93%, var(--primary-color, #4acadb));
}

.review-layout {
  height: calc(100vh - 150px);
  display: grid;
  grid-template-columns: 340px 1fr 360px;
  gap: 10px;
}

.review-stat-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.review-stat-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.stat-cell {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 86%, transparent);
}

.stat-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.stat-value {
  margin-top: 4px;
  font-size: 24px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.mastery-bar-track {
  height: 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-border) 75%, transparent);
  overflow: hidden;
}

.mastery-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #4acadb 0%, #6ed4a1 100%);
}

.review-card-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.review-topbar {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}

.review-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 14px;
  padding: 16px;
  background: linear-gradient(160deg, color-mix(in srgb, var(--guaxi-panel-bg) 95%, #7cc8ff 5%) 0%, color-mix(in srgb, var(--guaxi-panel-bg) 95%, #87f6c9 5%) 100%);
  min-height: 420px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.review-card-type {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.review-card-front {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 16px;
  min-height: 150px;
  background: var(--guaxi-panel-bg);
}

.review-word {
  font-size: 40px;
  font-weight: 700;
  line-height: 1.08;
}

.review-phonetic {
  margin-top: 10px;
  font-size: 14px;
  color: var(--guaxi-text-secondary);
}

.review-text {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.7;
  font-size: 18px;
}

.review-listening-player-wrap {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.review-listening-player {
  width: 100%;
}

.review-card-answer {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 14px;
  background: linear-gradient(160deg, color-mix(in srgb, var(--guaxi-header-bg) 74%, transparent) 0%, color-mix(in srgb, var(--guaxi-panel-bg) 92%, #8fb8ff 8%) 100%);
}

.review-answer-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.review-answer-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.review-answer-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 8px;
}

.review-answer-chip {
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  padding: 3px 10px;
  font-size: 12px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.review-answer-text {
  line-height: 1.65;
  font-size: 15px;
  white-space: pre-wrap;
  word-break: break-word;
}

.review-transcript-list {
  max-height: 280px;
}

.review-examples {
  margin-top: 10px;
}

.review-example-item {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 8px 10px;
  margin-top: 6px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
}

.review-answer-target {
  margin-top: 4px;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 72%, var(--guaxi-text-primary));
}

.review-actions {
  margin-top: auto;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.audio-form-card {
  overflow: auto;
}

@keyframes cardPop {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.random-card-head-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 8px;
}

.random-card-stars {
  font-size: 14px;
  color: #f5a524;
  letter-spacing: 2px;
}

.random-card-meta-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.phonetic-group {
  display: flex;
  align-items: center;
  gap: 2px;
}

.phonetic-tag {
  background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent);
}

.tag-trans {
  opacity: 0.8;
  font-size: 90%;
}

.random-card-section {
  margin-bottom: 12px;
}

.random-card-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 4px;
}

.random-card-content {
  line-height: 1.6;
  font-size: 14px;
}
</style>
