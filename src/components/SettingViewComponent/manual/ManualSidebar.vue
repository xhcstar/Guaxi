<template>
  <div class="manual-nav">
    <header class="mn-head">
      <div class="mn-kicker">FIELD MANUAL</div>
      <div class="mn-title">手册导航</div>

      <button type="button" class="mn-search" @click="manualStore.focusSearch()">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="7"></circle>
          <path d="m16.2 16.2 4.3 4.3"></path>
        </svg>
        <span>搜索手册</span>
        <kbd>Ctrl K</kbd>
      </button>

      <div class="mn-progress" aria-hidden="true">
        <span :style="{ width: `${manualStore.progress}%` }"></span>
      </div>
      <div class="mn-current">
        <span class="mn-current-no">{{ manualStore.activeChapter.number }}</span>
        <span class="mn-current-title">{{ manualStore.activeChapter.title }}</span>
        <span class="mn-current-pct">{{ Math.round(manualStore.progress) }}%</span>
      </div>
    </header>

    <div class="mn-scroll">
      <section class="mn-block">
        <div class="mn-block-title">全部章节</div>
        <template v-for="group in manualGroups" :key="group">
          <div class="mn-group">{{ group }}</div>
          <button
            v-for="chapter in chaptersByGroup(group)"
            :key="chapter.id"
            type="button"
            class="mn-chapter"
            :class="{ active: manualStore.activeChapterId === chapter.id }"
            @click="goChapter(chapter)"
          >
            <span class="mn-no">{{ chapter.number }}</span>
            <span class="mn-label">{{ chapter.title }}</span>
          </button>
        </template>
      </section>

      <section class="mn-block">
        <div class="mn-block-title">本章小节</div>
        <button
          v-for="(section, index) in manualStore.activeChapter.sections"
          :key="section.id"
          type="button"
          class="mn-section"
          :class="{ active: manualStore.activeSectionId === section.id }"
          @click="goSection(section.id)"
        >
          <span class="mn-section-no">{{ sectionNumber(index) }}</span>
          <span>{{ section.title }}</span>
        </button>
      </section>
    </div>

    <footer class="mn-foot">
      <span>点击章节跳转</span>
      <span>{{ manualStats.sections }} 个小节</span>
    </footer>
  </div>
</template>

<script setup>
import { manualChapters, manualGroups, manualStats } from './manualContent.js'
import { useManualStore } from '../../../stores/manualStore.js'

const manualStore = useManualStore()

const chaptersByGroup = (group) => manualChapters.filter((chapter) => chapter.group === group)

const sectionNumber = (index) =>
  `${manualStore.activeChapter.number}.${String(index + 1).padStart(2, '0')}`

const goChapter = (chapter) => {
  manualStore.requestScroll(chapter.id, { smooth: false })
}

const goSection = (sectionId) => {
  manualStore.requestScroll(sectionId, { smooth: true })
}
</script>

<style scoped>
.manual-nav {
  --mn-accent: #4078f2;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--guaxi-text-primary);
  background: var(--guaxi-panel-bg);
}

:global(:root[data-guaxi-theme='dark']) .manual-nav {
  --mn-accent: #61afef;
}

.mn-head {
  flex: 0 0 auto;
  padding: 16px 16px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.mn-kicker {
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 9.5px;
  letter-spacing: 0.16em;
  color: var(--guaxi-text-secondary);
}

.mn-title {
  margin-top: 4px;
  font-size: 17px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.mn-search {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  margin-top: 12px;
  padding: 7px 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-secondary);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: border-color 0.16s ease, color 0.16s ease;
}

.mn-search:hover {
  border-color: var(--mn-accent);
  color: var(--guaxi-text-primary);
}

.mn-search svg {
  flex: 0 0 auto;
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
}

.mn-search span {
  flex: 1 1 auto;
  text-align: left;
}

.mn-search kbd {
  padding: 1px 5px;
  border: 1px solid var(--guaxi-border);
  border-radius: 4px;
  background: var(--guaxi-panel-bg);
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 9.5px;
}

.mn-progress {
  height: 3px;
  margin-top: 12px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-text-primary) 10%, transparent);
  overflow: hidden;
}

.mn-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--mn-accent);
  transition: width 0.12s linear;
}

.mn-current {
  display: flex;
  align-items: baseline;
  gap: 7px;
  margin-top: 7px;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

.mn-current-no {
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 10px;
  color: var(--mn-accent);
}

.mn-current-title {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--guaxi-text-primary);
}

.mn-current-pct {
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 10px;
}

.mn-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 10px 10px 20px;
  scrollbar-width: thin;
}

.mn-block + .mn-block {
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px dashed var(--guaxi-border);
}

.mn-block-title {
  padding: 0 6px 6px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  color: var(--guaxi-text-secondary);
}

.mn-group {
  padding: 10px 6px 3px;
  font-size: 10px;
  letter-spacing: 0.08em;
  color: var(--guaxi-text-secondary);
  opacity: 0.8;
}

.mn-chapter,
.mn-section {
  display: flex;
  align-items: baseline;
  gap: 7px;
  width: 100%;
  border: 0;
  border-left: 2px solid transparent;
  border-radius: 0 6px 6px 0;
  background: transparent;
  color: var(--guaxi-text-secondary);
  text-align: left;
  font: inherit;
  cursor: pointer;
  transition: color 0.14s ease, background 0.14s ease, border-color 0.14s ease;
}

.mn-chapter {
  padding: 6px 7px;
  font-size: 12px;
  line-height: 1.45;
}

.mn-section {
  padding: 5px 7px;
  font-size: 11.5px;
  line-height: 1.5;
}

.mn-chapter:hover,
.mn-section:hover {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-text-primary) 6%, transparent);
}

.mn-chapter.active {
  border-left-color: var(--mn-accent);
  background: color-mix(in srgb, var(--mn-accent) 13%, transparent);
  color: var(--mn-accent);
  font-weight: 600;
}

.mn-section.active {
  border-left-color: var(--mn-accent);
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--mn-accent) 8%, transparent);
}

.mn-no {
  flex: 0 0 19px;
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 9.5px;
  opacity: 0.7;
}

.mn-label {
  flex: 1 1 auto;
  min-width: 0;
}

.mn-section-no {
  flex: 0 0 34px;
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 9px;
  color: var(--mn-accent);
  opacity: 0.85;
}

.mn-foot {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  gap: 8px;
  padding: 9px 14px;
  border-top: 1px solid var(--guaxi-border);
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
}
</style>
