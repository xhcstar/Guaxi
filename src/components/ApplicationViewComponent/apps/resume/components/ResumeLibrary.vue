<template>
  <aside class="rs-rail">
    <div class="rs-rail-head">
      <div class="rs-rail-title-row">
        <span class="rs-rail-title">{{ trashMode ? '回收站' : '简历库' }}</span>
        <span class="rs-rail-count">{{ trashMode ? store.deletedDocs.length : store.docCount }}</span>
      </div>
      <label class="rs-search">
        <Icon :icon="searchIcon" width="14" height="14" />
        <input v-model="keyword" type="search" placeholder="搜索标题或岗位" spellcheck="false" />
        <button v-if="keyword" type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--sm" aria-label="清除搜索" @click="keyword = ''">
          <Icon :icon="xIcon" width="12" height="12" />
        </button>
      </label>
    </div>

    <div class="rs-rail-list">
      <div
        v-for="(item, index) in filtered"
        :key="item.id"
        class="rs-doc"
        :class="{ 'is-active': !trashMode && item.id === store.activeDocId }"
        :style="{ '--i': index }"
        role="button"
        tabindex="0"
        @click="onCardOpen(item)"
        @keydown.enter="onCardOpen(item)"
      >
        <span class="rs-doc-spine" :style="{ background: spineColor(item) }" />
        <span class="rs-doc-main">
          <span class="rs-doc-title">
            {{ item.title }}
          </span>
          <span class="rs-doc-role">{{ item.role || '未填写求职意向' }}</span>
          <span class="rs-doc-meta">
            <span>{{ relativeTime(trashMode ? item.deletedAt : item.updatedAt) }}</span>
            <span>·</span>
            <span>{{ item.sectionCount }} 个模块</span>
            <Icon v-if="item.favorite && !trashMode" :icon="starIcon" width="11" height="11" class="rs-doc-star" />
          </span>
        </span>

        <span v-if="trashMode" class="rs-doc-actions is-static" @click.stop>
          <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="恢复到简历库" aria-label="恢复" @click="restore(item)">
            <Icon :icon="restoreIcon" width="13" height="13" />
          </button>
          <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--danger" title="彻底删除" aria-label="彻底删除" @click="purge(item)">
            <Icon :icon="trashIcon" width="12" height="12" />
          </button>
        </span>

        <span v-else class="rs-doc-actions" @click.stop>
          <button
            type="button"
            class="rs-btn rs-btn--ghost rs-btn--icon"
            :title="item.favorite ? '取消置顶' : '置顶'"
            :aria-label="item.favorite ? '取消置顶' : '置顶'"
            @click="toggleFavorite(item)"
          >
            <Icon :icon="starIcon" width="12" height="12" :style="{ opacity: item.favorite ? 1 : .45 }" />
          </button>
          <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="创建副本" aria-label="创建副本" @click="duplicate(item)">
            <Icon :icon="copyIcon" width="12" height="12" />
          </button>
          <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--danger" title="移到回收站" aria-label="移到回收站" @click="remove(item)">
            <Icon :icon="trashIcon" width="12" height="12" />
          </button>
        </span>
      </div>

      <div v-if="!filtered.length" class="rs-rail-empty">
        <template v-if="keyword">没有匹配「{{ keyword }}」的{{ trashMode ? '记录' : '简历' }}</template>
        <template v-else-if="trashMode">回收站是空的<br />删除的简历会先放在这里</template>
        <template v-else>简历库还是空的<br />从新建或导入开始</template>
      </div>
    </div>

    <div class="rs-rail-foot">
      <template v-if="trashMode">
        <button type="button" class="rs-btn" @click="trashMode = false">
          <Icon :icon="backIcon" width="14" height="14" />
          返回简历库
        </button>
      </template>
      <template v-else>
        <button type="button" class="rs-btn rs-btn--primary" :disabled="store.booting" @click="createDoc">
          <Icon :icon="plusIcon" width="14" height="14" />
          新建简历
        </button>
        <button type="button" class="rs-btn" @click="$emit('import')">
          <Icon :icon="uploadIcon" width="14" height="14" />
          导入项目文件
        </button>
        <button type="button" class="rs-btn" :disabled="!store.deletedDocs.length" @click="openTrash">
          <Icon :icon="trashIcon" width="14" height="14" />
          回收站
          <span v-if="store.deletedDocs.length" class="rs-trash-count">{{ store.deletedDocs.length }}</span>
        </button>
      </template>
      <span class="rs-storage-hint" :title="store.storagePath">
        {{ store.storagePath ? `本地存储 · ${store.storagePath}` : '本地 SQLite 存储' }}
      </span>
    </div>
  </aside>
</template>

<script setup>
import { computed, ref } from 'vue'
import { Icon } from '@iconify/vue'
import searchIcon from '@iconify-icons/lucide/search'
import xIcon from '@iconify-icons/lucide/x'
import plusIcon from '@iconify-icons/lucide/plus'
import copyIcon from '@iconify-icons/lucide/copy'
import trashIcon from '@iconify-icons/lucide/trash-2'
import starIcon from '@iconify-icons/lucide/star'
import uploadIcon from '@iconify-icons/lucide/upload'
import rotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import arrowLeftIcon from '@iconify-icons/lucide/arrow-left'
import { useResumeStore, relativeTime } from '../store/resumeStore'
import { askConfirm } from '../ui'
import { BUILTIN_TEMPLATES } from '../render/templates'

defineEmits(['import'])

const store = useResumeStore()
const keyword = ref('')
const trashMode = ref(false)
const restoreIcon = rotateCcwIcon
const backIcon = arrowLeftIcon

const sourceList = computed(() => (trashMode.value ? store.deletedDocs : store.docs))

const filtered = computed(() => {
  const query = keyword.value.trim().toLowerCase()
  if (!query) return sourceList.value
  return sourceList.value.filter((item) => (
    String(item.title || '').toLowerCase().includes(query)
    || String(item.role || '').toLowerCase().includes(query)
  ))
})

function onCardOpen(item) {
  if (!trashMode.value) open(item)
}

async function openTrash() {
  keyword.value = ''
  await store.loadDeletedDocs()
  trashMode.value = true
}

async function restore(item) {
  await store.restoreDoc(item.id)
}

async function purge(item) {
  const ok = await askConfirm({
    title: '彻底删除？',
    body: `「${item.title}」及其所有版本会被永久删除，无法恢复。`,
    confirmText: '永久删除',
    danger: true,
  })
  if (!ok) return
  await store.purgeDoc(item.id)
}

function spineColor(item) {
  const template = BUILTIN_TEMPLATES.find((entry) => entry.id === item.templateId)
  return template?.accent || '#8a6d4b'
}

function open(item) {
  store.openDoc(item.id)
}

async function createDoc() {
  await store.createDocument({ sample: false })
}

async function duplicate(item) {
  await store.duplicateDoc(item.id)
}

async function toggleFavorite(item) {
  await store.toggleFavorite(item.id, !item.favorite)
}

async function remove(item) {
  const ok = await askConfirm({
    title: '移到回收站？',
    body: `「${item.title}」会从简历库移除，版本记录一并隐藏。`,
    confirmText: '移到回收站',
    danger: true,
  })
  if (!ok) return
  await store.deleteDoc(item.id)
}
</script>
