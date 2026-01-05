<template>
  <div class="placeholder-root">
    <div class="placeholder-card">
      <div class="placeholder-title">应用占位</div>
      <div class="placeholder-desc">
        这里是该应用的占位界面。你可以后续为它单独创建一个更完整的组件。
      </div>
    </div>
  </div>
</template>

<script setup>
</script>

<style scoped>
.placeholder-root {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.placeholder-card {
  padding: 16px 20px;
  border-radius: 12px;
  background: #f9fafb;
  box-shadow: 0 4px 16px rgba(15, 23, 42, 0.08);
  max-width: 360px;
}

.placeholder-title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
}

.placeholder-desc {
  font-size: 13px;
  color: #6b7280;
}
</style>