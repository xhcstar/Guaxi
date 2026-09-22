<template>
  <div class="cog-right">
    <n-card title="兴趣画像" size="small" class="cog-right-card">
      <template #header-extra>
        <n-button text size="tiny" :loading="cog.profileLoading" @click="reload">
          <n-icon :component="ArrowSync20Regular" />
        </n-button>
      </template>

      <p class="cog-right-desc">
        这里的兴趣直接影响抓取的相关性排序与大脑的显著性打分。加几个你关心的主题，
        无关内容会被降权。
      </p>

      <!-- 新增兴趣 -->
      <n-input-group class="cog-add-row">
        <n-input v-model:value="newTopic" size="small" placeholder="添加兴趣主题，如 大模型推理优化"
          @keyup.enter="addInterest" />
        <n-button size="small" type="primary" :disabled="!newTopic.trim()" @click="addInterest">加入</n-button>
      </n-input-group>

      <!-- 兴趣列表 -->
      <div v-if="interests.length" class="cog-int-list">
        <div v-for="it in interests" :key="it.topic" class="cog-int-row">
          <span class="cog-int-topic" :title="it.topic">{{ it.topic }}</span>
          <span class="cog-int-src" :class="it.source">{{ it.source === 'manual' ? '手动' : '习得' }}</span>
          <span class="cog-int-weight" :title="`权重 ${it.weight.toFixed(2)}`">
            <i class="cog-int-bar"><b :style="{ width: weightPct(it.weight) }"></b></i>
          </span>
          <n-button text size="tiny" title="移除" @click="removeInterest(it.topic)">
            <n-icon :component="Dismiss20Regular" />
          </n-button>
        </div>
      </div>
      <div v-else class="cog-right-empty">
        还没有兴趣主题。手动添加，或让系统从你的阅读行为里逐步习得。
      </div>
    </n-card>

    <n-card title="专长与项目" size="small" class="cog-right-card" v-if="expertise.length || projects.length">
      <div v-if="expertise.length" class="cog-chip-group">
        <span class="cog-chip-label">专长</span>
        <n-tag v-for="e in expertise" :key="e" size="small" :bordered="false">{{ e }}</n-tag>
      </div>
      <div v-if="projects.length" class="cog-chip-group">
        <span class="cog-chip-label">项目</span>
        <n-tag v-for="p in projects" :key="p" size="small" :bordered="false" type="info">{{ p }}</n-tag>
      </div>
      <p class="cog-right-desc">在设置 → 认知里编辑完整画像（profile.md）。</p>
    </n-card>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { NCard, NButton, NIcon, NInput, NInputGroup, NTag, useMessage } from 'naive-ui'
import { ArrowSync20Regular, Dismiss20Regular } from '@vicons/fluent'
import { useCognitionStore } from '../../stores/cognitionStore'

const cog = useCognitionStore()
const message = useMessage()
const newTopic = ref('')

const interests = computed(() => {
  const list = Array.isArray(cog.profile?.interests) ? cog.profile.interests : []
  // 权重降序，手动的排前面。
  return [...list].sort((a, b) => {
    if ((a.source === 'manual') !== (b.source === 'manual')) return a.source === 'manual' ? -1 : 1
    return (b.weight || 0) - (a.weight || 0)
  })
})
const expertise = computed(() => cog.profile?.expertise || [])
const projects = computed(() => cog.profile?.projects || [])

const weightPct = (w) => `${Math.min(100, Math.round((Number(w) || 0) * 100))}%`

const reload = () => cog.loadProfile()

const addInterest = async () => {
  const topic = newTopic.value.trim()
  if (!topic) return
  try {
    await cog.upsertInterest(topic, 0.8)
    newTopic.value = ''
    message.success('已加入兴趣', { duration: 1000 })
  } catch (e) {
    message.error(String(e))
  }
}

const removeInterest = async (topic) => {
  try {
    await cog.removeInterest(topic)
    message.success('已移除', { duration: 1000 })
  } catch (e) {
    message.error(String(e))
  }
}

onMounted(() => {
  if (!cog.profile) cog.loadProfile()
})
</script>

<style scoped>
.cog-right {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 8px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, monospace;
}

.cog-right-desc {
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary, #64748b);
  margin: 0 0 10px;
}

.cog-add-row {
  margin-bottom: 10px;
}

.cog-int-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cog-int-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cog-int-topic {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}

.cog-int-src {
  flex: 0 0 auto;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 8px;
  background: rgba(127, 127, 127, 0.16);
  color: var(--guaxi-text-secondary, #64748b);
}

.cog-int-src.manual {
  background: rgba(45, 212, 191, 0.16);
  color: var(--n-color-primary, #2dd4bf);
}

.cog-int-weight {
  flex: 0 0 60px;
}

.cog-int-bar {
  display: block;
  height: 5px;
  border-radius: 3px;
  background: rgba(127, 127, 127, 0.22);
  overflow: hidden;
}

.cog-int-bar b {
  display: block;
  height: 100%;
  background: var(--n-color-primary, #2dd4bf);
}

.cog-right-empty {
  font-size: 13px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary, #64748b);
  padding: 8px 2px;
}

.cog-chip-group {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
}

.cog-chip-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}
</style>
