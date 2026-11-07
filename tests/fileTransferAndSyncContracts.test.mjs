// 传输中心与同步内核的契约测试。
//
// 这里锁的是“功能是否真的接上”：命令注册、鉴权头、文件夹相对路径、P2P 直传、
// 分片上传、接收任务入队，以及传输中心 UI 的关键入口。和仓库里其它 contract
// 测试一样，读源码做静态断言，不启动 Tauri。
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

const mainRs = read('src-tauri/src/main.rs')
const transferRs = read('src-tauri/src/commands/filetransfer.rs')
const p2pRs = read('src-tauri/src/commands/p2psync.rs')
const settingRs = read('src-tauri/src/commands/settingview.rs')
const transferVue = read('src/components/ApplicationViewComponent/apps/FileTransferAssistantApp.vue')
const syncVue = read('src/components/SettingViewComponent/SyncEngineSettings.vue')
const transferSettingVue = read('src/components/SettingViewComponent/TransferContent.vue')
const syncMod = read('src-tauri/src/commands/sync/mod.rs')
const cloudRs = read('src-tauri/src/commands/sync/cloud.rs')
const localRs = read('src-tauri/src/commands/sync/local.rs')
const s3Rs = read('src-tauri/src/commands/sync/s3.rs')
const engineRs = read('src-tauri/src/commands/sync/engine.rs')

test('传输命令已注册到 Tauri handler', () => {
  for (const command of [
    'filetransfer_send_entries',
    'filetransfer_collect_entries',
    'filetransfer_clear_finished',
    'filetransfer_retry_task',
    'p2p_send_files',
  ]) {
    assert.match(mainRs, new RegExp(`commands::(filetransfer|p2psync)::${command}`), command)
  }
  for (const command of [
    'sync_engine_get_config',
    'sync_engine_set_config',
    'sync_engine_test_connection',
    'sync_engine_run_target',
    'sync_engine_run_all',
  ]) {
    assert.match(mainRs, new RegExp(`commands::sync::${command}`), command)
  }
})

test('接收端鉴权、文件夹相对路径与任务入队', () => {
  assert.match(transferRs, /x-guaxi-token/)
  assert.match(transferRs, /fn token_allowed/)
  assert.match(transferRs, /relative_path:\s*String/)
  assert.match(transferRs, /fn sanitize_relative_path/)
  assert.match(transferRs, /fn unique_relative_path/)
  assert.match(transferRs, /UPLOAD_TASK_MAP/)
  assert.match(transferRs, /direction:\s*"recv"/)
  assert.match(transferRs, /SHARE_READ_ONLY/)
  assert.match(transferRs, /fn signal_token_allowed/)
})

test('传输中心包含设备栏、拖拽、重试与 P2P 直传入口', () => {
  assert.match(transferVue, /class="ftc-root"/)
  assert.match(transferVue, /P2P 已配对/)
  assert.match(transferVue, /const setupDropTarget/)
  assert.match(transferVue, /filetransfer_collect_entries/)
  assert.match(transferVue, /filetransfer_retry_task/)
  assert.match(transferVue, /p2p_send_files/)
  assert.match(transferVue, /p2p-file-progress/)
  assert.match(transferVue, /receiverToken/)
  assert.match(transferVue, /filetransfer_list_local_ips/)
  assert.match(transferVue, /selectedLocalUrl/)
  assert.match(transferVue, /toggleHttpServer/)
  assert.match(transferRs, /filetransfer_list_local_ips/)
  assert.match(mainRs, /filetransfer_list_local_ips/)
})

test('WebRTC 信令配对码前后端都接通', () => {
  assert.match(settingRs, /webrtc_signal_token/)
  assert.match(settingRs, /get_webrtc_signal_token/)
  assert.match(settingRs, /set_webrtc_signal_token/)
  assert.match(transferSettingVue, /webrtcSignalToken/)
  assert.match(transferVue, /webrtcSignalToken/)
  assert.match(mainRs, /get_webrtc_signal_token/)
})

test('P2P 大文件优化：块大小、传输窗口、哈希复用、推送续传', () => {
  assert.match(p2pRs, /P2P_CHUNK_SIZE:\s*usize\s*=\s*2\s*\*\s*1024\s*\*\s*1024/)
  assert.match(p2pRs, /guaxi\/p2p-sync\/3/)
  assert.match(p2pRs, /max_concurrent_bidi_streams/)
  assert.match(p2pRs, /stream_receive_window/)
  assert.match(p2pRs, /receive_window/)
  assert.match(p2pRs, /P2P_TRANSFER_MAX_RETRIES:\s*u32\s*=\s*8/)
  assert.match(p2pRs, /push_part_path/)
  assert.match(p2pRs, /entry\.blake3\.clone\(\)/)
  assert.match(p2pRs, /received/)
  assert.match(p2pRs, /PARALLEL_PULL_THRESHOLD/)
  assert.match(p2pRs, /pull_file_parallel/)
  assert.match(p2pRs, /P2P_DELTA_MAX_SIZE/)
  assert.match(p2pRs, /update_mmap/)
  assert.match(p2pRs, /known_hash/)
  assert.match(p2pRs, /FileProgress/)
  assert.match(p2pRs, /let jitter = rand::random/)
  assert.match(p2pRs, /is_sync_cancelled\(&progress\.run_id\)/)
  assert.match(p2pRs, /EndpointAddr::from/)
  assert.match(p2pRs, /PARALLEL_PUSH_THRESHOLD/)
  assert.match(p2pRs, /PARALLEL_PUSH_RANGES/)
  assert.match(p2pRs, /push_file_parallel/)
  assert.match(p2pRs, /PushFileComplete/)
  assert.match(p2pRs, /assemble_push_ranges/)
  assert.match(p2pRs, /push_range_marker/)
  assert.match(p2pRs, /PushTransferMeta/)
  assert.match(p2pRs, /transfer_meta_path/)
  assert.match(p2pRs, /push_transfer_file_parallel/)
  assert.match(p2pRs, /transfer_id/)
  assert.match(p2pRs, /transfer_with_retry\(&link/)
  assert.match(p2pRs, /P2P_IO_TIMEOUT_SECS/)
  assert.match(p2pRs, /发送文件体超时/)
  assert.match(p2pRs, /接收文件体超时/)
  assert.match(p2pRs, /"transfers"/)
})

test('同步内核包含 WebDAV / S3 / SFTP / 本地四类后端', () => {
  for (const kind of ['webdav', 's3', 'sftp', 'local']) {
    assert.match(syncMod, new RegExp(`"${kind}"`), kind)
  }
  assert.match(localRs, /impl SyncBackend for LocalBackend/)
  assert.match(s3Rs, /put_file_multipart/)
  assert.match(s3Rs, /MULTIPART_THRESHOLD/)
  assert.match(engineRs, /SyncOp::Conflict/)
  assert.match(engineRs, /delete_missing/)
})

test('同步目标设置页提供五种连接类型', () => {
  for (const kind of ['webdav', 's3', 'sftp', 'local', 'cloud']) {
    assert.match(syncVue, new RegExp(`value:\\s*'${kind}'`), kind)
  }
  assert.match(syncVue, /sync_engine_set_config/)
  assert.match(syncVue, /sync:progress/)
})

test('网盘直连：百度 / 阿里云盘 / 夸克配置与后端分支', () => {
  assert.match(syncMod, /"cloud"/)
  assert.match(syncMod, /CloudConnection/)
  assert.match(cloudRs, /BAIDU_API/)
  assert.match(cloudRs, /ALIYUN_API/)
  assert.match(cloudRs, /QUARK_API/)
  assert.match(cloudRs, /refresh_baidu/)
  assert.match(cloudRs, /refresh_aliyun/)
  assert.match(cloudRs, /put_baidu/)
  assert.match(cloudRs, /put_aliyun/)
  assert.match(cloudRs, /夸克官方上传 API 尚未公开/)
  assert.match(syncVue, /百度网盘/)
  assert.match(syncVue, /阿里云盘/)
  assert.match(syncVue, /夸克/)
})
