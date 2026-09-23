<template>
  <transition name="queue-slide">
    <div v-if="store.queueVisible" class="queue-drawer">
      <div class="qd-head">
        <div class="qd-title">播放队列 <span class="qd-count">{{ store.queue.length }}</span></div>
        <button class="qd-clear" :disabled="!store.queue.length" @click="store.clearQueue()">清空</button>
      </div>
      <div class="qd-list">
        <div v-if="!store.queue.length" class="qd-empty">队列是空的</div>
        <div
          v-for="(t, i) in store.queue"
          :key="t.id + '_' + i"
          class="qd-item"
          :class="{ active: i === store.currentIndex }"
          @dblclick="store.playAt(i)"
        >
          <div class="qd-item-main" @click="store.playAt(i)">
            <div class="qd-item-title">{{ t.title }}</div>
            <div class="qd-item-artist">{{ t.artist || "未知歌手" }}</div>
          </div>
          <button class="qd-remove" title="移除" @click.stop="store.removeFromQueue(i)">
            <n-icon :component="CloseOutline" :size="15" />
          </button>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup>
import { NIcon } from "naive-ui";
import { CloseOutline } from "@vicons/ionicons5";
import { useMusicStore } from "../../../../stores/useMusicStore";
const store = useMusicStore();
</script>

<style scoped>
.queue-drawer {
  position: absolute;
  right: 0; top: 0; bottom: 72px;
  width: 320px;
  background: var(--guaxi-panel-bg, #fff);
  border-left: 1px solid var(--guaxi-border, #e5e7eb);
  box-shadow: -8px 0 24px rgba(0,0,0,0.08);
  display: flex; flex-direction: column;
  z-index: 20;
}
.qd-head { display: flex; align-items: center; justify-content: space-between; padding: 16px 18px; border-bottom: 1px solid var(--guaxi-border, #eee); }
.qd-title { font-size: 15px; font-weight: 700; color: var(--guaxi-text-primary); }
.qd-count { font-size: 13px; color: var(--guaxi-text-secondary); font-weight: 400; margin-left: 4px; }
.qd-clear { border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font-size: 13px; }
.qd-clear:hover:not(:disabled) { color: #a78bfa; }
.qd-clear:disabled { opacity: 0.4; cursor: not-allowed; }
.qd-list { flex: 1; overflow: auto; padding: 8px; }
.qd-empty { text-align: center; color: var(--guaxi-text-secondary); font-size: 14px; padding: 40px 0; }
.qd-item { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 8px; cursor: pointer; }
.qd-item:hover { background: rgba(128,128,128,0.14); }
.qd-item:hover .qd-remove { opacity: 1; }
.qd-item.active .qd-item-title { color: #a78bfa; font-weight: 600; }
.qd-item-main { flex: 1; min-width: 0; }
.qd-item-title { font-size: 14px; color: var(--guaxi-text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.qd-item-artist { font-size: 12px; color: var(--guaxi-text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.qd-remove { opacity: 0; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; display: flex; transition: opacity 0.15s; }
.qd-remove:hover { color: #ef4444; }

.queue-slide-enter-active, .queue-slide-leave-active { transition: transform 0.22s ease, opacity 0.22s ease; }
.queue-slide-enter-from, .queue-slide-leave-to { transform: translateX(100%); opacity: 0.6; }
</style>
