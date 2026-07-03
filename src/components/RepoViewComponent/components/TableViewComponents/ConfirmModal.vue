<template>
  <Teleport to="body">
    <transition name="cm-fade">
      <div v-if="confirmState.open" class="cm-mask" @mousedown.self="onCancel">
        <div class="cm-dialog" role="alertdialog" aria-modal="true">
          <div class="cm-band" :class="{ danger: confirmState.danger }">
            <n-icon :size="16" class="cm-band-icon">
              <Warning20Filled />
            </n-icon>
            <span class="cm-band-text">{{ confirmState.danger ? '删除确认' : '请确认' }}</span>
          </div>

          <div class="cm-body">
            <h3 class="cm-title">{{ confirmState.title }}</h3>
            <p v-if="confirmState.description" class="cm-desc">{{ confirmState.description }}</p>
            <div v-if="confirmState.count != null" class="cm-stat">
              <span class="cm-stat-num">{{ confirmState.count }}</span>
              <span class="cm-stat-unit">{{ confirmState.countUnit }}将被删除</span>
            </div>
          </div>

          <div class="cm-actions">
            <button class="cm-btn ghost" @click="onCancel">{{ confirmState.cancelLabel }}</button>
            <button class="cm-btn danger" @click="onConfirm">{{ confirmState.confirmLabel }}</button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<script setup>
import { NIcon } from 'naive-ui'
import { Warning20Filled } from '@vicons/fluent'
import { useConfirm } from '../../composables/useConfirm.js'

const { confirmState, onConfirm, onCancel } = useConfirm()
</script>

<style scoped>
.cm-mask {
  position: fixed;
  inset: 0;
  z-index: 12000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.42);
  backdrop-filter: blur(2px);
}

.cm-dialog {
  width: 380px;
  max-width: calc(100vw - 40px);
  border-radius: 14px;
  overflow: hidden;
  background: var(--guaxi-panel-bg, #1f2430);
  border: 1px solid var(--guaxi-border, rgba(255, 255, 255, 0.1));
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
}

.cm-band {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 18px;
  font-size: 13px;
  font-weight: 600;
  color: #e6a23c;
  background: rgba(230, 162, 60, 0.12);
  border-bottom: 1px solid rgba(230, 162, 60, 0.2);
}
.cm-band.danger {
  color: #e5484d;
  background: rgba(229, 72, 77, 0.12);
  border-bottom-color: rgba(229, 72, 77, 0.22);
}
.cm-band-icon {
  display: inline-flex;
}

.cm-body {
  padding: 18px 18px 8px;
}
.cm-title {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--guaxi-text-primary, #e6e6e6);
  line-height: 1.4;
  word-break: break-word;
}
.cm-desc {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary, #9aa0aa);
}
.cm-stat {
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--guaxi-header-bg, rgba(255, 255, 255, 0.04));
  display: flex;
  align-items: baseline;
  gap: 6px;
}
.cm-stat-num {
  font-size: 20px;
  font-weight: 700;
  color: #e5484d;
}
.cm-stat-unit {
  font-size: 13px;
  color: var(--guaxi-text-secondary, #9aa0aa);
}

.cm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 18px 18px;
}
.cm-btn {
  min-width: 76px;
  height: 34px;
  padding: 0 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.16s ease;
  border: 1px solid transparent;
}
.cm-btn.ghost {
  background: transparent;
  border-color: var(--guaxi-border, rgba(255, 255, 255, 0.14));
  color: var(--guaxi-text-primary, #d0d0d0);
}
.cm-btn.ghost:hover {
  background: var(--guaxi-header-bg, rgba(255, 255, 255, 0.06));
}
.cm-btn.danger {
  background: #e5484d;
  color: #fff;
}
.cm-btn.danger:hover {
  background: #d13c41;
  box-shadow: 0 4px 12px rgba(229, 72, 77, 0.32);
}

.cm-fade-enter-active,
.cm-fade-leave-active {
  transition: opacity 0.18s ease;
}
.cm-fade-enter-from,
.cm-fade-leave-to {
  opacity: 0;
}
</style>
