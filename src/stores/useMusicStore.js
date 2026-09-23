// src/stores/useMusicStore.js
import { defineStore } from "pinia";
import { createLogger } from "../utils/logger";

const logger = createLogger("useMusicStore");

// 播放模式：顺序播完即停 / 列表循环 / 单曲循环 / 随机
export const PLAY_MODES = ["order", "list", "single", "shuffle"];

export const useMusicStore = defineStore("music", {
  state: () => ({
    // 播放队列：Track[] { id, title, artist, album, duration, cover, url, source }
    queue: [],
    currentIndex: -1,
    isPlaying: false,
    currentTime: 0,
    duration: 0,
    volume: 0.8,
    muted: false,
    playMode: "list",
    // 随机模式下已播过的索引，避免重复
    shuffleHistory: [],
    // 队列抽屉开关
    queueVisible: false,
    // 沉浸播放页开关
    nowPlayingVisible: false,
  }),

  getters: {
    currentTrack: (state) =>
      state.currentIndex >= 0 ? state.queue[state.currentIndex] ?? null : null,
    hasTrack: (state) => state.currentIndex >= 0 && state.queue.length > 0,
    playModeLabel: (state) =>
      ({ order: "顺序播放", list: "列表循环", single: "单曲循环", shuffle: "随机播放" }[
        state.playMode
      ]),
  },

  actions: {
    // 用一组曲目替换队列并从 startIndex 播放
    setQueue(tracks, startIndex = 0) {
      this.queue = Array.isArray(tracks) ? tracks.slice() : [];
      this.shuffleHistory = [];
      if (this.queue.length === 0) {
        this.currentIndex = -1;
        this.isPlaying = false;
        return;
      }
      this.currentIndex = Math.min(Math.max(0, startIndex), this.queue.length - 1);
      this.isPlaying = true;
      this.currentTime = 0;
    },

    playAt(index) {
      if (index < 0 || index >= this.queue.length) return;
      this.currentIndex = index;
      this.isPlaying = true;
      this.currentTime = 0;
    },

    togglePlay() {
      if (!this.hasTrack) return;
      this.isPlaying = !this.isPlaying;
    },

    setPlaying(v) {
      this.isPlaying = !!v;
    },

    next(auto = false) {
      if (this.queue.length === 0) return;
      // 单曲循环且是自动触发（播完）：原地重播
      if (auto && this.playMode === "single") {
        this.currentTime = 0;
        this.isPlaying = true;
        return;
      }
      if (this.playMode === "shuffle") {
        this._playRandom();
        return;
      }
      const last = this.currentIndex >= this.queue.length - 1;
      if (last) {
        if (this.playMode === "list" || this.playMode === "single") {
          this.currentIndex = 0;
        } else {
          // order：播到底自动停
          if (auto) {
            this.isPlaying = false;
            return;
          }
          this.currentIndex = 0;
        }
      } else {
        this.currentIndex += 1;
      }
      this.currentTime = 0;
      this.isPlaying = true;
    },

    prev() {
      if (this.queue.length === 0) return;
      if (this.playMode === "shuffle") {
        this._playRandom();
        return;
      }
      this.currentIndex =
        this.currentIndex <= 0 ? this.queue.length - 1 : this.currentIndex - 1;
      this.currentTime = 0;
      this.isPlaying = true;
    },

    _playRandom() {
      if (this.queue.length === 1) {
        this.currentTime = 0;
        this.isPlaying = true;
        return;
      }
      let next;
      let guard = 0;
      do {
        next = Math.floor(Math.random() * this.queue.length);
        guard += 1;
      } while (next === this.currentIndex && guard < 20);
      this.currentIndex = next;
      this.currentTime = 0;
      this.isPlaying = true;
    },

    cyclePlayMode() {
      const i = PLAY_MODES.indexOf(this.playMode);
      this.playMode = PLAY_MODES[(i + 1) % PLAY_MODES.length];
    },

    setPlayMode(mode) {
      if (PLAY_MODES.includes(mode)) this.playMode = mode;
    },

    seek(time) {
      this.currentTime = Math.max(0, Math.min(time, this.duration || 0));
      this._seekRequest = { time: this.currentTime, ts: Date.now() };
    },

    setVolume(v) {
      this.volume = Math.max(0, Math.min(1, v));
      if (this.volume > 0) this.muted = false;
    },

    toggleMute() {
      this.muted = !this.muted;
    },

    // 播放器上报
    reportTime(t) {
      this.currentTime = t;
    },
    reportDuration(d) {
      this.duration = d || 0;
    },

    addToQueue(track, playNow = false) {
      this.queue.push(track);
      if (playNow) this.playAt(this.queue.length - 1);
    },

    removeFromQueue(index) {
      if (index < 0 || index >= this.queue.length) return;
      this.queue.splice(index, 1);
      if (index < this.currentIndex) {
        this.currentIndex -= 1;
      } else if (index === this.currentIndex) {
        if (this.queue.length === 0) {
          this.currentIndex = -1;
          this.isPlaying = false;
        } else {
          this.currentIndex = Math.min(this.currentIndex, this.queue.length - 1);
          this.currentTime = 0;
        }
      }
    },

    clearQueue() {
      this.queue = [];
      this.currentIndex = -1;
      this.isPlaying = false;
      this.currentTime = 0;
      this.duration = 0;
    },

    toggleQueueDrawer() {
      this.queueVisible = !this.queueVisible;
    },

    toggleNowPlaying() {
      this.nowPlayingVisible = !this.nowPlayingVisible;
    },
  },
});
