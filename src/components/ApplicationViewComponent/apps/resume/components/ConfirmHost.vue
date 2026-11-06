<template>
  <div v-if="confirmState.open" class="rs-scrim" style="z-index: 260;" @click.self="settleConfirm(false)">
    <div class="rs-modal rs-modal--compact" role="dialog" aria-modal="true">
      <div class="rs-modal-head">
        <div class="rs-grow">
          <h2>{{ confirmState.title }}</h2>
          <p v-if="confirmState.body">{{ confirmState.body }}</p>
        </div>
      </div>
      <div class="rs-modal-foot">
        <button type="button" class="rs-btn" @click="settleConfirm(false)">{{ confirmState.cancelText }}</button>
        <button
          type="button"
          class="rs-btn"
          :class="confirmState.danger ? 'rs-btn--danger' : 'rs-btn--primary'"
          @click="settleConfirm(true)"
        >{{ confirmState.confirmText }}</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { confirmState, settleConfirm } from '../ui'
</script>
