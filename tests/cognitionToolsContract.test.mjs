import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

// 认知层工具面的**契约**：这几条不是实现细节，是 codex 能不能调到工具的前提。
//
// 背景（实测得出，见 tools.rs / mcp.rs 的注释）：
//   - codex 0.149 在 approval_policy=never 下只放行带 readOnlyHint 的工具，
//     缺 annotations 的 MCP 调用会全部被拒；
//   - 工具描述被写进 JSON 帧，中文必须原样通过（桥接已改为按字节读写）；
//   - 每次 GUI/写入类动作都要能追溯到「要不要问用户」，这条由 requiresConfirm 表达。
//
// 这个测试直接读源码做静态断言：它不需要跑起 Tauri，也不会因为某台机器上
// 某个服务没启动而跳过——契约坏了就是坏了。

const here = dirname(fileURLToPath(import.meta.url))
const srcRoot = join(here, '..', 'src-tauri', 'src', 'cognition')

const read = (p) => readFileSync(join(srcRoot, p), 'utf8')

/** 从 REGISTRY 里抽出每条 spec 的 (name, tier, description)。 */
function parseRegistry() {
  const text = read('tools.rs')
  const out = []
  const re = /spec\(\s*"([a-z0-9_]+)",\s*Tier::(\w+),/g
  let m
  while ((m = re.exec(text))) {
    out.push({ name: m[1], tier: m[2].toLowerCase() })
  }
  return out
}

const registry = parseRegistry()

test('工具名唯一且只出现在一个域模块里', () => {
  assert.ok(registry.length >= 40, `注册表只有 ${registry.length} 个工具`)
  const names = registry.map((t) => t.name)
  assert.equal(new Set(names).size, names.length, '有重名工具')

  const modules = ['kb', 'write', 'net', 'exec', 'gui']
  const seen = new Map()
  for (const mod of modules) {
    const text = read(`tools/${mod}.rs`)
    for (const name of names) {
      if (text.includes(`"${name}"`)) {
        assert.ok(!seen.has(name), `${name} 同时出现在 ${seen.get(name)} 与 ${mod}`)
        seen.set(name, mod)
      }
    }
  }
  const missing = names.filter((n) => !seen.has(n))
  assert.deepEqual(missing, [], `这些工具没有域模块负责分发: ${missing.join(', ')}`)
})

test('等级授权覆盖全部工具，且 read 恒开', () => {
  const text = read('tools.rs')
  assert.match(text, /Tier::Read => true/, 'read 级必须恒开')
  // tier_grants 必须遍历 Tier::ALL，而不是手写清单——手写清单会在加等级时漏。
  assert.match(text, /Tier::ALL\s*\n?\s*\.iter\(\)/, 'tier_grants 应遍历 Tier::ALL')
})

test('未授权等级不会出现在 list_tools 的 enabled 里', () => {
  const text = read('tools.rs')
  const fn = text.slice(text.indexOf('pub fn list_tools()'))
  assert.match(fn, /t\.enabled = granted\(t\.tier\)/, 'list_tools 必须按授权状态刷新 enabled')
})

test('MCP tools/list 输出 readOnlyHint 等 annotations', () => {
  const text = read('mcp.rs')
  assert.match(text, /"readOnlyHint":\s*t\.tier == tools::Tier::Read/)
  assert.match(text, /"openWorldHint":\s*t\.tier == tools::Tier::Network/)
  assert.match(text, /"destructiveHint"/)
})

test('GUI 与写入类工具要求逐动作确认，只读与出网不要求', () => {
  const text = read('tools.rs')
  const m = text.match(/requires_confirm:\s*matches!\(tier,\s*([^)]+)\)/)
  assert.ok(m, 'requires_confirm 需要由 tier 决定')
  const tiers = m[1].split('|').map((s) => s.trim())
  assert.deepEqual(tiers.sort(), ['Tier::Gui', 'Tier::Write'].sort())
})

test('逐动作确认只对带 UI 的调用方生效', () => {
  const text = read('tools.rs')
  assert.match(text, /fn caller_needs_confirm\(caller: &str\) -> bool \{\s*matches!\(caller, "codex" \| "agent"\)/)
  // dispatch 里必须在执行前判断，而不是事后补审计。
  const dispatch = text.slice(text.indexOf('pub async fn dispatch('))
  assert.ok(
    dispatch.indexOf('confirm_tool') < dispatch.indexOf('execute(name, args)'),
    '确认必须发生在执行之前',
  )
})

test('桥接按字节读写，不依赖平台编码', () => {
  const text = read('mcp_bridge.rs')
  assert.match(text, /read_until\(b'\\n'/, '必须按字节读到换行')
  assert.match(text, /write_all\(line\.as_bytes\(\)\)/, '必须按字节写出')
  assert.doesNotMatch(text, /writeln!\(/, '不能再用按平台编码的 writeln!')
})

test('codex 配置带审批模式与工具白名单', () => {
  const agent = readFileSync(
    join(here, '..', 'src-tauri', 'src', 'commands', 'codex_agent.rs'),
    'utf8',
  )
  assert.match(agent, /default_tools_approval_mode = \\"\{approval_mode\}\\"|default_tools_approval_mode/,
    '必须写 default_tools_approval_mode，否则写类工具永远被拒')
  assert.match(agent, /enabled_tools = /, '未授权工具不应出现在模型视野里')
  assert.match(agent, /startup_timeout_sec = 60/, '首次拉起桥接需要更长的启动超时')
})

test('出网授权决定 codex 原生 web_search 开关', () => {
  const agent = readFileSync(
    join(here, '..', 'src-tauri', 'src', 'commands', 'codex_agent.rs'),
    'utf8',
  )
  assert.match(agent, /web_search = \\"\{web_search\}\\"|web_search = "?\{web_search\}/)
  assert.match(agent, /Tier::Network/)
})

test('联网抓取工具存在且复用既有抓取管线', () => {
  const net = read('tools/net.rs')
  assert.match(net, /"guaxi_web_fetch"/)
  assert.match(net, /flipboard_render::fetch_page/)
  assert.match(net, /flipboard_extract::extract/)
  // 正文必须包不可信声明，否则网页里的指令会被模型当成用户指令。
  assert.match(net, /untrusted_block/)
})

test('截图能作为图片回到模型（dynamicTools inputImage）', () => {
  const text = readFileSync(
    join(here, '..', 'src-tauri', 'src', 'commands', 'codex_app_server.rs'),
    'utf8',
  )
  assert.match(text, /"type": "inputImage"/)
  assert.match(text, /dynamicTools/)
})

// ── 分发与启动路径 ──────────────────────────────
//
// 这两条对应真实踩过的两个坑，任何一个复发都会让「模型没有工具」原样重演，
// 而现象看起来完全不像打包问题。

test('不把 guaxi_mcp 声明成 externalBin', () => {
  // externalBin 在**编译期**就要求那个带目标三元组后缀的文件存在。
  // 而它是构建产物、不该进版本库，于是任何新检出直接 `cargo build` 都会
  // 因为「resource path ... doesn't exist」失败。工具面改走主程序的
  // `--mcp-server` 模式（下面那条测试保证它不再 panic）。
  const conf = JSON.parse(
    readFileSync(join(here, '..', 'src-tauri', 'tauri.conf.json'), 'utf8'),
  )
  assert.equal(
    conf.bundle?.externalBin,
    undefined,
    '不要让 externalBin 依赖一个不在版本库里的构建产物',
  )
})

test('--mcp-server 在 tokio 运行时之外分叉', () => {
  // 桥接用 reqwest::blocking，在异步上下文里跑会在退出时 panic
  // （Cannot drop a runtime...）；这条分叉必须在普通 fn main 里。
  const text = readFileSync(join(here, '..', 'src-tauri', 'src', 'main.rs'), 'utf8')
  const lines = text.split(/\r?\n/)
  const asyncAttr = lines.findIndex((l) => l.trim() === '#[tokio::main]')
  const syncMain = lines.findIndex((l) => l.trim() === 'fn main() {')
  const mcpBranch = lines.findIndex((l) => l.includes('"--mcp-server"'))
  assert.ok(syncMain >= 0, 'main.rs 需要一个同步 fn main 作为入口')
  assert.ok(mcpBranch > syncMain, '--mcp-server 的分叉必须在同步 fn main 里')
  assert.ok(
    asyncAttr > mcpBranch,
    '异步入口（app_main）应在 --mcp-server 分叉之后，不能被它包住',
  )
})
