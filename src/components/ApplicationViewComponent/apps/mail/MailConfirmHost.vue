<template>
  <div v-if="mailConfirmState.open" class="mail-modal-backdrop" @mousedown.self="answerMailConfirm(false)">
    <div class="mail-modal mail-confirm" role="dialog" aria-modal="true">
      <div class="mail-modal-body">
        <p v-if="mailConfirmState.title" class="mail-confirm-title">{{ mailConfirmState.title }}</p>
        <p class="mail-confirm-message">{{ mailConfirmState.message }}</p>
      </div>
      <div class="mail-modal-foot">
        <button
          class="mail-btn"
          :class="mailConfirmState.danger ? 'is-danger' : 'is-primary'"
          @click="answerMailConfirm(true)"
        >
          {{ mailConfirmState.confirmText }}
        </button>
        <button class="mail-btn" @click="answerMailConfirm(false)">取消</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted } from 'vue'
import { answerMailConfirm, mailConfirmState } from './mailConfirm'

/** Esc = 取消。确认框没有输入，只要一个明确的二选一。 */
function onKey(event) {
  if (!mailConfirmState.open) return
  if (event.key === 'Escape') {
    event.stopPropagation()
    answerMailConfirm(false)
  } else if (event.key === 'Enter') {
    event.stopPropagation()
    answerMailConfirm(true)
  }
}

onMounted(() => document.addEventListener('keydown', onKey, true))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey, true))
</script>
