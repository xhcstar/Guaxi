// codex ⇄ Guaxi MCP 工具面回归探针。
//
// 用途：每次升级 codex 版本（`npm run codex:fetch:force`）后跑一次，确认「模型真的
// 能用上 Guaxi 的工具」这条链路没有断。这里检查的每一环都曾经单独坏过：
//
//   1. `codex mcp list` 里 guaxi 是 enabled —— 桥接能被拉起、握手能完成；
//   2. 只读工具在 approval_policy=never 下能跑 —— 依赖 tools/list 的 readOnlyHint；
//   3. 写工具在 default_tools_approval_mode=auto 下被拒、在 prompt 下产生审批请求
//      —— 这是「不会偷偷改用户的文件」的保证。
//
// 需要一个能用的模型供应商（默认复用 Guaxi 已生成的 config.toml 与 cc-switch）。
// 模型不可用时 1、2、3 中的协议断言仍会给出明确失败原因，而不是笼统的「超时」。
//
// 用法：
//   node tools/codex_mcp_probe.mjs
//   node tools/codex_mcp_probe.mjs --home <CODEX_HOME>   # 默认复用应用生成的那份
//   node tools/codex_mcp_probe.mjs --offline             # 只跑第 1 项（不调模型）
//
// 退出码：0 全部通过；1 有检查项失败（会把原始报文打出来）。

import { spawn, spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'

const projectRoot = process.cwd()
const exeName = process.platform === 'win32' ? 'codex.exe' : 'codex'
const binary = path.join(projectRoot, 'src-tauri', 'utils', 'codex', exeName)
const bridgeName = process.platform === 'win32' ? 'guaxi_mcp.exe' : 'guaxi_mcp'
const bridge =
  [path.join(projectRoot, 'src-tauri', 'target', 'release', bridgeName),
   path.join(projectRoot, 'src-tauri', 'target', 'debug', bridgeName)]
    .find((p) => existsSync(p)) || path.join(projectRoot, 'src-tauri', 'target', 'release', bridgeName)

const argValue = (name) => {
  const i = process.argv.indexOf(name)
  return i >= 0 ? process.argv[i + 1] : null
}
const offline = process.argv.includes('--offline')

const results = []
const record = (name, ok, detail = '') => {
  results.push({ name, ok, detail })
  console.log(`${ok ? '  ok  ' : ' FAIL '} ${name}${detail ? ` — ${detail}` : ''}`)
}

if (!existsSync(binary)) {
  console.error(`找不到 codex: ${binary}\n先跑 npm run codex:fetch`)
  process.exit(1)
}

// 复用的 home：优先命令行给的，否则用应用生成的那份（里面已有可用的供应商配置）。
// 应用生成的那份：dev 在 target/release/config，安装版在 <安装目录>/config，
// 便携版可能就放在 D:/guaxi/config。逐个试，找到含 config.toml 的那份。
const candidates = [
  argValue('--home'),
  path.join(projectRoot, 'src-tauri', 'target', 'release', 'config', 'codex-home'),
  path.join(projectRoot, 'src-tauri', 'target', 'debug', 'config', 'codex-home'),
  process.env.GUAXI_CODEX_HOME,
].filter(Boolean)
const appHome = candidates.find((p) => existsSync(path.join(p, 'config.toml')))
// 永远在临时目录里工作：探针要改 config.toml 的 MCP 段，绝不能在用户
// 正在用的那份上动手。有现成 home 就复制一份（含供应商配置），否则只建空的。
const home = mkdtempSync(path.join(tmpdir(), 'guaxi-mcp-probe-'))
if (appHome) {
  cpSync(appHome, home, { recursive: true, filter: (src) => !src.includes('sessions') })
}
const useTempHome = true

console.log(`codex : ${binary}`)
console.log(`bridge: ${bridge}${existsSync(bridge) ? '' : '（缺失，将退回主程序 --mcp-server）'}`)
console.log(`home  : ${home}`)

// 把 MCP 段换成探针要用的那份，其余配置（模型/供应商）保持原样。
const baseConfigPath = path.join(home, 'config.toml')
const baseConfig = existsSync(baseConfigPath) ? readFileSync(baseConfigPath, 'utf8') : ''
const baseWithoutMcp = baseConfig.replace(/\n\[mcp_servers\.guaxi\][\s\S]*?(?=\n\[|$)/, '')

const hasBridge = existsSync(bridge)
const command = hasBridge ? bridge : process.execPath
const args = hasBridge ? [] : ['--mcp-server']

function writeConfig(approvalMode) {
  if (!baseConfig.trim()) {
    // 没有任何基准配置：写一份最小可用的（依赖环境里的 GUAXI_CODEX_API_KEY）。
    writeFileSync(baseConfigPath, [
      'model = "deepseek/deepseek-v4.1-flash-1017"',
      'model_provider = "guaxi"',
      'approval_policy = "never"',
      'sandbox_mode = "read-only"',
      'disable_response_storage = true',
      '',
      '[model_providers.guaxi]',
      'name = "Guaxi (probe)"',
      'base_url = "http://127.0.0.1:15721/v1"',
      'wire_api = "responses"',
      'env_key = "GUAXI_CODEX_API_KEY"',
      '',
    ].join(String.fromCharCode(10)), 'utf8')
  }
  const lines = [
    baseWithoutMcp.trimEnd(),
    '',
    '[mcp_servers.guaxi]',
    `command = ${JSON.stringify(command)}`,
    args.length ? `args = [${args.map((a) => JSON.stringify(a)).join(', ')}]` : 'args = []',
    'startup_timeout_sec = 60',
    `default_tools_approval_mode = "${approvalMode}"`,
    '',
  ]
  writeFileSync(baseConfigPath, lines.join('\n'), 'utf8')
}

// ── 1. codex 认得 guaxi 这个 MCP 服务器吗
writeConfig('auto')
const listed = spawnSync(binary, ['mcp', 'list'], {
  env: { ...process.env, CODEX_HOME: home },
  encoding: 'utf8',
  timeout: 60_000,
})
const listLine = String(listed.stdout || '')
  .split(/\r?\n/)
  .find((l) => l.trimStart().startsWith('guaxi'))
record(
  'codex mcp list 注册了 guaxi 且状态 enabled',
  Boolean(listLine && listLine.includes('enabled')),
  listLine ? listLine.trim() : `原始输出: ${String(listed.stdout || listed.stderr).trim()}`,
)

if (offline) {
  finish()
}

// ── 2/3. 用真模型跑一轮，看只读与写入工具的实际待遇
function runExec(prompt, approvalMode) {
  writeConfig(approvalMode)
  const res = spawnSync(
    binary,
    ['exec', '--json', '--skip-git-repo-check', '-s', 'read-only', '-'],
    {
      env: { ...process.env, CODEX_HOME: home, GUAXI_CODEX_API_KEY: process.env.GUAXI_CODEX_API_KEY || 'guaxi-managed' },
      input: prompt,
      encoding: 'utf8',
      timeout: 300_000,
    },
  )
  if (process.env.GUAXI_PROBE_VERBOSE) {
    console.log('[stdout]', res.stdout)
    console.log('[stderr]', res.stderr)
  }
  const items = []
  for (const line of String(res.stdout || '').split(/\r?\n/)) {
    if (!line.trim().startsWith('{')) continue
    try {
      const evt = JSON.parse(line)
      if (evt.item) items.push(evt.item)
      if (evt.type === 'turn.failed') items.push({ type: 'error', message: evt.error?.message })
    } catch {
      // 非 JSON 行（启动警告）忽略
    }
  }
  return items
}

const readItems = runExec(
  '只调用一次 MCP 工具 guaxi_kb_list，然后结束。不要调用其它工具。',
  'auto',
)
// 必须取**最后一条**同工具的记录：item.started 也是 mcp_tool_call 且 status=in_progress，
// 取第一条永远看到「还在跑」。
const readCalls = readItems.filter((i) => i.type === 'mcp_tool_call' && i.tool === 'guaxi_kb_list')
const readCall = readCalls[readCalls.length - 1]
const readOk = Boolean(readCall && readCall.status === 'completed' && !readCall.error)
let readDetail = ''
if (!readCall) {
  const err = readItems.find((i) => i.type === 'error')
  readDetail = err?.message || '模型没有调用该工具（先确认供应商可用）'
} else if (!readOk) {
  readDetail = readCall.error?.message || `调用失败（状态 ${readCall.status}）`
}
record('只在 approval=never 下可被调用（readOnlyHint 生效）', readOk, readDetail)

const writeItems = runExec(
  '只调用一次 MCP 工具 guaxi_kb_add_tag（kbName=Computer, nodeId=codex_probe, tag=probe），然后结束。',
  'auto',
)
const writeCalls = writeItems.filter((i) => i.type === 'mcp_tool_call' && i.tool === 'guaxi_kb_add_tag')
const writeCall = writeCalls[writeCalls.length - 1]
const writeBlocked = Boolean(writeCall && writeCall.status === 'failed' && /approval/i.test(writeCall.error?.message || ''))
record(
  '写工具在 approval=auto 下被要求审批（不会静默写入）',
  writeBlocked,
  writeCall ? (writeCall.error?.message || `状态: ${writeCall.status}`) : '模型没有调用写工具（先确认供应商可用）',
)

finish()

function finish() {
  if (useTempHome) {
    try {
      rmSync(home, { recursive: true, force: true })
    } catch {
      // 清理失败不影响结论
    }
  }
  const failed = results.filter((r) => !r.ok)
  console.log('')
  console.log(failed.length === 0 ? '全部通过' : `${failed.length} 项失败`)
  process.exit(failed.length === 0 ? 0 : 1)
}
