<template>
  <n-modal
    :show="show"
    @update:show="(v) => emit('update:show', v)"
    :mask-closable="true"
    :auto-focus="false"
  >
    <div class="share-modal-card">
      <div class="smc-head">
        <span class="smc-title">资料库 · 分享与发布</span>
        <button class="smc-close" @click="emit('update:show', false)">×</button>
      </div>
      <div class="smc-body">
        <LibrarySiteApp v-if="show" />
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { NModal } from "naive-ui";
import LibrarySiteApp from "../ApplicationViewComponent/apps/LibrarySiteApp.vue";

defineProps({ show: { type: Boolean, default: false } });
const emit = defineEmits(["update:show"]);
</script>

<style scoped>
.share-modal-card {
  width: 92vw;
  height: 88vh;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-app-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
}
.smc-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}
.smc-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}
.smc-close {
  border: none;
  background: none;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
}
.smc-close:hover {
  color: var(--guaxi-text-primary);
}
.smc-body {
  flex: 1;
  min-height: 0;
  position: relative;
}
</style>
