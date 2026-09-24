<template>
  <div class="rhi" :class="[view, 'k-'+item.kind, { active, sel: selected }]" @click="clickable && $emit('pick', item)">
    <label v-if="selectable" class="rhi-check" @click.stop>
      <n-checkbox :checked="selected" @update:checked="v => $emit('toggle', item.id, v)" />
    </label>
    <img v-if="view==='grid' && thumb" class="rhi-thumb" :src="thumb" alt="" loading="lazy" />
    <div class="rhi-body">
      <div class="rhi-title">
        <span class="rhi-badge" :class="'k-'+item.kind"><Icon :icon="kindIcon" width="12" height="12" /></span>
        <span class="rhi-name" :title="item.title">{{ item.title }}</span>
      </div>
      <div class="rhi-meta">
        <span>{{ item.source || '本地' }}</span>
        <span v-if="item.format" class="rhi-fmt">{{ String(item.format).toUpperCase() }}</span>
        <span v-if="item.size_hint" class="rhi-fmt">{{ fmtSize(item.size_hint) }}</span>
        <span v-if="item.needs_ytdlp" class="rhi-tag">yt-dlp</span>
        <span v-if="item.magnet" class="rhi-tag">磁力</span>
        <span v-for="t in extraTags" :key="t" class="rhi-tag">{{ t }}</span>
        <span v-if="typeof item.relevance === 'number'" class="rhi-rel">{{ (item.relevance * 100).toFixed(0) }}%</span>
      </div>
    </div>
    <div class="rhi-actions" @click.stop>
      <n-tooltip v-for="a in actions" :key="a.key" :delay="200">
        <template #trigger>
          <button class="rhi-btn" :class="a.cls" @click="$emit('act', a.key, item)"><Icon :icon="a.icon" width="15" height="15" /></button>
        </template>{{ a.tip }}
      </n-tooltip>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import fileIcon from '@iconify-icons/lucide/file'

const props = defineProps({
  item: { type: Object, required: true },
  view: { type: String, default: 'list' },
  kindMeta: { type: Object, default: () => ({}) },
  actions: { type: Array, default: () => [] },
  selectable: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  clickable: { type: Boolean, default: false },
  active: { type: Boolean, default: false },
  thumb: { type: String, default: '' },
})
defineEmits(['toggle', 'act', 'pick'])

const kindIcon = computed(() => props.kindMeta[props.item.kind]?.icon || fileIcon)
const extraTags = computed(() => {
  const t = props.item.tags
  if (Array.isArray(t)) return t
  if (typeof t === 'string') return t.split(',').map((s) => s.trim()).filter(Boolean)
  return []
})
function fmtSize(b) {
  if (!b) return ''
  if (b > 1073741824) return (b / 1073741824).toFixed(1) + ' GB'
  if (b > 1048576) return (b / 1048576).toFixed(1) + ' MB'
  if (b > 1024) return (b / 1024).toFixed(0) + ' KB'
  return b + ' B'
}
</script>

<style scoped>
.rhi { display: flex; align-items: center; gap: 10px; border-left: 3px solid var(--k-color, transparent); box-sizing: border-box; }
.rhi.list { padding: 9px 20px; border-bottom: 1px solid var(--guaxi-border, #eee); height: 100%; }
.rhi.grid { flex-direction: column; align-items: stretch; padding: 12px; border: 1px solid var(--guaxi-border, #e6e9ee); border-radius: 10px; background: var(--guaxi-panel-bg, #fafbfc); }
.rhi.k-paper { --k-color: var(--k-paper); } .rhi.k-book { --k-color: var(--k-book); }
.rhi.k-video { --k-color: var(--k-video); } .rhi.k-audio { --k-color: var(--k-audio); }
.rhi.k-image { --k-color: var(--k-image); } .rhi.k-code { --k-color: var(--k-code); } .rhi.k-other { --k-color: var(--k-other); }
.rhi.active { background: color-mix(in srgb, var(--rh-accent, #2563eb) 8%, transparent); }
.rhi[clickable], .rhi.sel { cursor: pointer; }
.rhi-check { flex-shrink: 0; }
.rhi-body { flex: 1; min-width: 0; }
.rhi-title { display: flex; align-items: center; gap: 7px; font-size: 14px; line-height: 1.4; color: var(--guaxi-text-primary, #1f2328); }
.rhi.list .rhi-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rhi.grid .rhi-name { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.rhi-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 12px; color: var(--guaxi-text-secondary, #57606a); margin-top: 4px; }
.rhi-fmt { font-weight: 600; letter-spacing: .3px; }
.rhi-tag { padding: 0 6px; border-radius: 4px; background: var(--guaxi-border, #eef); font-size: 11px; }
.rhi-rel { margin-left: auto; }
.rhi.grid .rhi-rel { margin-left: 0; }
.rhi-actions { display: flex; gap: 4px; flex-shrink: 0; }
.rhi.grid .rhi-actions { justify-content: flex-end; margin-top: 8px; }
.rhi-thumb { width: 100%; height: 120px; object-fit: cover; border-radius: 8px; margin-bottom: 8px; background: var(--guaxi-border, #eee); }
.rhi-badge { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: 6px; color: #fff; flex-shrink: 0; background: var(--k-other); }
.rhi-badge.k-paper { background: var(--k-paper); } .rhi-badge.k-book { background: var(--k-book); }
.rhi-badge.k-video { background: var(--k-video); } .rhi-badge.k-audio { background: var(--k-audio); }
.rhi-badge.k-image { background: var(--k-image); } .rhi-badge.k-code { background: var(--k-code); } .rhi-badge.k-other { background: var(--k-other); }
.rhi-btn { width: 28px; height: 26px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--guaxi-text-secondary, #57606a); cursor: pointer; display: inline-flex; align-items: center; justify-content: center; }
.rhi-btn:hover { background: var(--guaxi-border, #eef); color: var(--guaxi-text-primary); }
.rhi-btn.primary { color: var(--rh-accent, #2563eb); }
.rhi-btn.danger:hover { background: color-mix(in srgb, var(--k-video, #ef4444) 14%, transparent); color: var(--k-video, #ef4444); }
</style>
