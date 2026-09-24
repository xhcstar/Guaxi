<template>
  <div class="lib-app">
    <div class="lib-topbar">
      <span class="lib-title">资料库 · 库网站</span>
      <div class="lib-actions">
        <button class="lib-btn" @click="reload" title="刷新">刷新</button>
        <button class="lib-btn primary" @click="openShare">分享</button>
      </div>
    </div>

    <iframe
      ref="frame"
      class="lib-frame"
      :src="localUrl"
      title="库网站"
    ></iframe>

    <!-- 分享抽屉 -->
    <transition name="drawer">
      <div v-if="shareOpen" class="share-mask" @click.self="shareOpen = false">
        <aside class="share-drawer">
          <header class="share-head">
            <h3>分享资料库</h3>
            <button class="share-close" @click="shareOpen = false">×</button>
          </header>

          <section class="share-body">
            <div class="share-toggle-row">
              <div>
                <div class="stitle">对外分享</div>
                <div class="ssub">开启后可用微信 / QQ 发送链接，或手机扫码在浏览器打开</div>
              </div>
              <button
                class="switch"
                :class="{ on: status.enabled }"
                @click="toggleShare"
                :disabled="busy"
              >
                <span class="knob"></span>
              </button>
            </div>

            <div v-if="status.enabled" class="share-card">
              <div class="qr-wrap" v-html="qrSvg"></div>
              <div class="link-row">
                <input class="link-input" :value="status.url" readonly @focus="selectAll" />
                <button class="lib-btn" @click="copyLink">复制</button>
              </div>
              <p class="share-hint">
                同一局域网内的设备可直接扫码 / 打开链接浏览、检索、下载。
                关闭分享会立即撤销该链接。
              </p>
            </div>

            <div v-else class="share-empty">
              <div class="empty-glyph">⇄</div>
              <p>尚未开启对外分享</p>
            </div>

            <div class="peer-section">
              <div class="share-toggle-row">
                <div>
                  <div class="stitle">开放联邦浏览</div>
                  <div class="ssub">允许已配对的其他 Guaxi 设备浏览本机库、按需取文件</div>
                </div>
                <button
                  class="switch"
                  :class="{ on: fedEnabled }"
                  @click="toggleFederation"
                  :disabled="fedBusy"
                >
                  <span class="knob"></span>
                </button>
              </div>
              <div class="peer-hint">
                在库网站左栏「联邦来源 · 发现」里可发现其他设备开放的库，
                合并浏览、阅读、下载。
              </div>
            </div>
          </section>
        </aside>
      </div>
    </transition>
  </div>
</template>
<!-- PLACEHOLDER_SCRIPT -->

<script setup>
import { ref, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";

const frame = ref(null);
const localUrl = ref("http://127.0.0.1:8888/lib/");
const shareOpen = ref(false);
const status = ref({ enabled: false, url: null, token: null });
const qrSvg = ref("");
const busy = ref(false);
const fedEnabled = ref(true);
const fedBusy = ref(false);
let themeObserver = null;

// 读取 Guaxi 当前主题(与全局 CSS 同一个数据源:documentElement 的 data-guaxi-theme),
// 保证内嵌库网站与 Guaxi 界面永远同亮同暗,不会半亮半暗。
function guaxiTheme() {
  return document.documentElement.dataset.guaxiTheme === "dark" ? "dark" : "light";
}

function pushThemeToFrame() {
  const win = frame.value && frame.value.contentWindow;
  if (win) win.postMessage({ type: "guaxi-theme", theme: guaxiTheme() }, "*");
}

onMounted(async () => {
  let base = "http://127.0.0.1:8888/lib/";
  try {
    base = await invoke("library_site_local_url");
  } catch (e) {
    // 命令未就绪时保留默认地址
  }
  // 初始就带上主题,iframe 首帧即与宿主一致。
  const u = new URL(base);
  u.searchParams.set("theme", guaxiTheme());
  localUrl.value = u.toString();

  // Guaxi 切换主题时,实时通知 iframe 跟随。
  themeObserver = new MutationObserver(() => pushThemeToFrame());
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-guaxi-theme"],
  });

  try {
    status.value = await invoke("library_site_share_status");
    if (status.value.enabled && status.value.url) await refreshQr();
  } catch (e) {}
  try {
    fedEnabled.value = await invoke("p2p_federation_status");
  } catch (e) {}
});

onBeforeUnmount(() => {
  if (themeObserver) themeObserver.disconnect();
});

async function toggleFederation() {
  if (fedBusy.value) return;
  fedBusy.value = true;
  try {
    fedEnabled.value = await invoke("p2p_federation_set_enabled", { enabled: !fedEnabled.value });
  } catch (e) {
    console.error("切换联邦浏览失败", e);
  } finally {
    fedBusy.value = false;
  }
}

function reload() {
  if (frame.value) frame.value.src = localUrl.value;
}

function openShare() {
  shareOpen.value = true;
}

async function refreshQr() {
  if (!status.value.url) return;
  try {
    qrSvg.value = await invoke("library_site_qr_svg", { text: status.value.url });
  } catch (e) {
    qrSvg.value = "";
  }
}

async function toggleShare() {
  if (busy.value) return;
  busy.value = true;
  try {
    if (status.value.enabled) {
      status.value = await invoke("library_site_stop_share");
      qrSvg.value = "";
    } else {
      status.value = await invoke("library_site_start_share");
      await refreshQr();
    }
  } catch (e) {
    console.error("切换分享失败", e);
  } finally {
    busy.value = false;
  }
}

function selectAll(e) {
  e.target.select();
}

async function copyLink() {
  if (!status.value.url) return;
  try {
    await navigator.clipboard.writeText(status.value.url);
  } catch (e) {}
}
</script>
<!-- PLACEHOLDER_STYLE -->

<style scoped>
.lib-app { display: flex; flex-direction: column; height: 100%; background: var(--guaxi-app-bg); }
.lib-topbar {
  flex: 0 0 auto; display: flex; align-items: center; justify-content: space-between;
  padding: 8px 14px; border-bottom: 1px solid var(--guaxi-border); background: var(--guaxi-panel-bg);
}
.lib-title { font-size: 14px; font-weight: 600; color: var(--guaxi-text-primary); }
.lib-actions { display: flex; gap: 8px; }
.lib-btn {
  font-size: 13px; cursor: pointer; padding: 5px 12px; border-radius: 7px;
  border: 1px solid var(--guaxi-border); background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary); transition: all 0.15s ease;
}
.lib-btn:hover { border-color: #1890ff; }
.lib-btn.primary { background: #1890ff; border-color: #1890ff; color: #fff; }
.lib-frame { flex: 1; width: 100%; border: none; background: var(--guaxi-app-bg); }

.share-mask { position: absolute; inset: 0; background: rgba(0,0,0,0.32); display: flex; justify-content: flex-end; z-index: 50; }
.share-drawer { width: 360px; max-width: 90%; height: 100%; background: var(--guaxi-panel-bg); border-left: 1px solid var(--guaxi-border); display: flex; flex-direction: column; box-shadow: -8px 0 24px rgba(0,0,0,0.18); }
.share-head { display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--guaxi-border); }
.share-head h3 { margin: 0; font-size: 15px; color: var(--guaxi-text-primary); }
.share-close { border: none; background: none; font-size: 22px; line-height: 1; cursor: pointer; color: var(--guaxi-text-secondary); }
.share-body { padding: 18px; overflow-y: auto; }

.share-toggle-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.stitle { font-size: 14px; font-weight: 600; color: var(--guaxi-text-primary); }
.ssub { font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 3px; line-height: 1.5; }
.switch { position: relative; width: 44px; height: 24px; border-radius: 12px; border: none; background: var(--guaxi-border); cursor: pointer; transition: background 0.2s ease; flex: 0 0 auto; }
.switch.on { background: #1890ff; }
.switch .knob { position: absolute; top: 2px; left: 2px; width: 20px; height: 20px; border-radius: 50%; background: #fff; transition: transform 0.2s ease; }
.switch.on .knob { transform: translateX(20px); }

.share-card { margin-top: 18px; }
.qr-wrap { display: grid; place-items: center; padding: 16px; border: 1px solid var(--guaxi-border); border-radius: 12px; background: #ffffff; }
.qr-wrap :deep(svg) { width: 200px; height: 200px; }
.link-row { display: flex; gap: 8px; margin-top: 12px; }
.link-input { flex: 1; min-width: 0; font-family: 'Maple Mono NF CN', Consolas, monospace; font-size: 12px; padding: 7px 10px; border: 1px solid var(--guaxi-border); border-radius: 7px; background: var(--guaxi-app-bg); color: var(--guaxi-text-primary); }
.share-hint { font-size: 12px; color: var(--guaxi-text-secondary); line-height: 1.6; margin-top: 12px; }
.share-empty { margin-top: 24px; text-align: center; color: var(--guaxi-text-secondary); }
.share-empty .empty-glyph { font-size: 40px; opacity: 0.4; }
.peer-section { margin-top: 28px; padding-top: 16px; border-top: 1px solid var(--guaxi-border); }
.peer-empty { font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 8px; }
.peer-hint { font-size: 12px; color: var(--guaxi-text-secondary); line-height: 1.6; margin-top: 12px; }

.drawer-enter-active, .drawer-leave-active { transition: opacity 0.2s ease; }
.drawer-enter-active .share-drawer, .drawer-leave-active .share-drawer { transition: transform 0.24s cubic-bezier(0.22,0.61,0.36,1); }
.drawer-enter-from, .drawer-leave-to { opacity: 0; }
.drawer-enter-from .share-drawer, .drawer-leave-to .share-drawer { transform: translateX(100%); }
</style>
