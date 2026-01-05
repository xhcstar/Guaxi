<template>
  <div class="music-player-root">
    <div class="player-header">
      <div class="player-title">音乐播放器</div>
      <div class="player-subtitle">播放列表 · 最近添加</div>
    </div>

    <div class="player-main">
      <div class="player-cover">
        <div class="player-cover-inner">
          🎵
        </div>
      </div>

      <div class="player-info">
        <div class="track-title">示例歌曲 - Track Title</div>
        <div class="track-artist">示例歌手 · Demo Artist</div>

        <div class="track-progress">
          <div class="track-time">01:23</div>
          <div class="track-bar">
            <div class="track-bar-inner"></div>
          </div>
          <div class="track-time">03:45</div>
        </div>

        <div class="player-controls">
          <n-button circle size="small" quaternary>⏮</n-button>
          <n-button circle type="primary">▶</n-button>
          <n-button circle size="small" quaternary>⏭</n-button>
        </div>
      </div>
    </div>

    <div class="playlist">
      <div class="playlist-title">播放列表</div>
      <div class="playlist-list">
        <div v-for="n in 6" :key="n" class="playlist-item">
          <div class="playlist-index">{{ n }}</div>
          <div class="playlist-meta">
            <div class="playlist-name">歌曲 {{ n }}</div>
            <div class="playlist-artist">歌手 {{ n }}</div>
          </div>
          <div class="playlist-duration">03:2{{ n }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
</script>

<style scoped>
.music-player-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 16px 18px;
  box-sizing: border-box;
}

.player-header {
  margin-bottom: 12px;
}

.player-title {
  font-size: 18px;
  font-weight: 600;
}

.player-subtitle {
  font-size: 12px;
  color: #6b7280;
}

.player-main {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}

.player-cover {
  width: 120px;
  height: 120px;
  border-radius: 16px;
  background: linear-gradient(135deg, #667eea, #764ba2);
  display: flex;
  align-items: center;
  justify-content: center;
}

.player-cover-inner {
  width: 80px;
  height: 80px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.16);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  color: #fff;
}

.player-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.track-title {
  font-weight: 600;
  font-size: 15px;
}

.track-artist {
  font-size: 12px;
  color: #6b7280;
}

.track-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: #6b7280;
}

.track-bar {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: #e5e7eb;
  overflow: hidden;
}

.track-bar-inner {
  width: 40%;
  height: 100%;
  background: linear-gradient(135deg, #34d399, #4ade80);
}

.player-controls {
  display: flex;
  align-items: center;
  gap: 10px;
}

.playlist {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: auto;
}

.playlist-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
}

.playlist-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.playlist-item {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
}

.playlist-item:hover {
  background: #f3f4f6;
}

.playlist-index {
  width: 24px;
  color: #9ca3af;
}

.playlist-meta {
  flex: 1;
}

.playlist-name {
  font-weight: 500;
}

.playlist-artist {
  color: #6b7280;
  font-size: 11px;
}

.playlist-duration {
  color: #6b7280;
}
</style>