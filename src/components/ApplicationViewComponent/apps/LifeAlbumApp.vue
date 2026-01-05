<template>
  <div class="life-album-root">
    <div class="album-header">
      <div class="album-title">相册</div>
      <div class="album-actions">
        <n-button size="small" tertiary>导入图片</n-button>
        <n-button size="small" secondary>新建相册</n-button>
      </div>
    </div>
    <div class="album-grid">
      <div v-for="n in 8" :key="n" class="album-item">
        <div class="album-thumb">
          <span class="album-thumb-icon">🖼️</span>
        </div>
        <div class="album-meta">
          <div class="album-name">相册 {{ n }}</div>
          <div class="album-desc">包含 {{ 5 + n }} 张照片</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
</script>

<style scoped>
.life-album-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px 16px;
  box-sizing: border-box;
}

.album-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.album-title {
  font-size: 18px;
  font-weight: 600;
}

.album-actions {
  display: flex;
  gap: 8px;
}

.album-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 12px;
  overflow: auto;
}

.album-item {
  background: #f9fafb;
  border-radius: 10px;
  padding: 10px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.album-thumb {
  height: 80px;
  border-radius: 8px;
  background: linear-gradient(135deg, #fbc2eb, #a6c1ee);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
}

.album-thumb-icon {
  font-size: 26px;
}

.album-meta {
  font-size: 12px;
}

.album-name {
  font-weight: 600;
  margin-bottom: 2px;
}

.album-desc {
  color: #6b7280;
}
</style>