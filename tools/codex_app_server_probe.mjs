// codex app-server 协议探针。
//
// 用途：每次升级 codex 版本后跑一次，确认工作台依赖的协议面没有变。
// 这里检查的每一项都是 `src-tauri/src/commands/codex_app_server.rs` 实际用到的：
//
//   1. initialize（experimentalApi=true）→ 必须成功，否则 dynamicTools 不可用
//   2. model/list                       → 返回 data 数组；inputModalities 决定能否看截图
//   3. thread/start                     → 返回 thread.id；sandbox 必须回 {type, networkAccess}
//   4. turn/start                       → 返回 turn.id，并产生 item/started / turn/completed
//
// 不连接真实模型供应商——只发一轮最常见的问候，验证事件形状。
// 若配置里没有可用供应商，第 4 项会失败，其余三项仍应通过。
//
// 用法：
//   node tools/codex_app_server_probe.mjs
//   node tools/codex_app_server_probe.mjs --home <CODEX_HOME>   # 默认用临时目录
//
// 退出码：0 全部通过；1 有检查项失败（会把失败的原始报文打出来）。

import { spawn } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'

const projectRoot = process.cwd()
const binary = path.join(projectRoot, 'src-tauri', 'utils', 'codex', process.platform === 'win32' ? 'codex.exe' : 'codex')

const homeArgIndex = process.argv.indexOf('--home')
const tempHome = homeArgIndex >= 0 ? null : mkdtempSync(path.join(tmpdir(), 'guaxi-codex-probe-'))
const codexHome = homeArgIndex >= 0 ? process.argv[homeArgIndex + 1] : tempHome

const results = []
const record = (name, ok, detail = '') => {
  results.push({ name, ok, detail })
  console.log(`${ok ? '  ok  ' : ' FAIL '} ${name}${detail ? ` — ${detail}` : ''}`)
}

const child = spawn(binary, ['app-server', '--stdio'], {
  env: { ...process.env, CODEX_HOME: codexHome },
  stdio: ['pipe', 'pipe', 'pipe'],
})

const inbox = []
const waiters = new Map()
let nextId = 1
let stdoutBuf = ''

const send = (method, params) => {
  const id = nextId++
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
  return id
}

const write = (obj) => child.stdin.write(`${JSON.stringify(obj)}\n`)

const notifications = []
child.stdout.on('data', (chunk) => {
  stdoutBuf += chunk.toString()
  let idx
  while ((idx = stdoutBuf.indexOf('\n')) >= 0) {
    const line = stdoutBuf.slice(0, idx).trim()
    stdoutBuf = stdoutBuf.slice(idx + 1)
    if (!line) continue
    let msg
    try {
      msg = JSON.parse(line)
    } catch {
      continue
    }
    const method = msg.method || ''
    if (method) {
      notifications.push(msg)
      if (msg.id !== undefined) {
        // 服务端请求（审批等）：这里一律拒绝，避免卡住。
        write({ jsonrpc: '2.0', id: msg.id, result: { decision: 'decline' } })
      }
      continue
    }
    const waiter = waiters.get(msg.id)
    if (waiter) {
      waiters.delete(msg.id)
      waiter(msg)
    }
  }
})

child.stderr.on('data', (chunk) => {
  const text = chunk.toString().trim()
  if (text) console.log(`[stderr] ${text}`)
})

const rpc = (method, params, timeoutMs = 20000) =>
  new Promise((resolve, reject) => {
    const id = send(method, params)
    const timer = setTimeout(() => {
      waiters.delete(id)
      reject(new Error(`${method} 超时（${timeoutMs}ms）`))
    }, timeoutMs)
    waiters.set(id, (msg) => {
      clearTimeout(timer)
      if (msg.error) reject(new Error(msg.error.message || JSON.stringify(msg.error)))
      else resolve(msg.result)
    })
  })

const waitFor = (method, timeoutMs = 60000) =>
  new Promise((resolve) => {
    const started = Date.now()
    const tick = () => {
      const hit = notifications.find((n) => n.method === method)
      if (hit) return resolve(hit)
      if (Date.now() - started > timeoutMs) return resolve(null)
      setTimeout(tick, 120)
    }
    tick()
  })

const cleanup = () => {
  try {
    child.kill()
  } catch {
    // 已经退出了
  }
  if (tempHome) {
    try {
      rmSync(tempHome, { recursive: true, force: true })
    } catch {
      // 临时目录清不掉不影响结论
    }
  }
}

const main = async () => {
  // 1) 握手
  try {
    const init = await rpc('initialize', {
      clientInfo: { name: 'guaxi-probe', version: '1' },
      capabilities: { experimentalApi: true },
    })
    record('initialize (experimentalApi)', Boolean(init?.userAgent), init?.userAgent || '')
  } catch (err) {
    record('initialize (experimentalApi)', false, err.message)
    cleanup()
    process.exitCode = 1
    return
  }
  write({ jsonrpc: '2.0', method: 'initialized', params: {} })

  // 2) 模型清单
  let model = ''
  try {
    const list = await rpc('model/list', {})
    const data = Array.isArray(list?.data) ? list.data : []
    const withImage = data.filter((m) => (m.inputModalities || []).includes('image')).length
    record('model/list', data.length > 0, `${data.length} 个模型，其中 ${withImage} 个支持图像输入`)
    model = data[0]?.model || data[0]?.id || ''
  } catch (err) {
    record('model/list', false, err.message)
  }

  // 3) 建线程
  let threadId = ''
  try {
    const params = { cwd: projectRoot, sandbox: 'read-only', approvalPolicy: 'on-request' }
    if (model) params.model = model
    const started = await rpc('thread/start', params)
    const thread = started?.thread || {}
    threadId = thread.id || ''
    const sandboxShape = started?.sandbox && typeof started.sandbox === 'object' && 'type' in started.sandbox
    record(
      'thread/start',
      Boolean(threadId) && sandboxShape,
      `id=${threadId.slice(0, 8)}… sandbox=${started?.sandbox?.type || '?'} networkAccess=${started?.sandbox?.networkAccess}`,
    )
  } catch (err) {
    record('thread/start', false, err.message)
  }

  // 4) 发一轮，等事件
  if (threadId) {
    try {
      const turn = await rpc('turn/start', {
        threadId,
        input: [{ type: 'text', text: 'Reply with exactly the word PONG.' }],
      })
      record('turn/start', Boolean(turn?.turn?.id), `turn=${String(turn?.turn?.id || '').slice(0, 8)}…`)

      const completed = await waitFor('turn/completed', 120000)
      if (completed) {
        const status = completed.params?.turn?.status
        record('turn/completed', status === 'completed', `status=${status}`)
      } else {
        record('turn/completed', false, '120s 内未收到；通常是供应商未配置，不影响协议面')
      }

      const kinds = new Set(notifications.map((n) => n.method))
      const wanted = ['item/started', 'item/completed', 'thread/started', 'turn/started']
      const missing = wanted.filter((k) => !kinds.has(k))
      record('事件面', missing.length === 0, missing.length ? `缺少 ${missing.join(', ')}` : `收到 ${kinds.size} 类事件`)
    } catch (err) {
      record('turn/start', false, err.message)
    }
  }

  cleanup()
  const failed = results.filter((r) => !r.ok)
  console.log(`\n${results.length - failed.length}/${results.length} 项通过`)
  process.exitCode = failed.length ? 1 : 0
}

main().catch((err) => {
  console.error('探针异常：', err)
  cleanup()
  process.exitCode = 1
})
