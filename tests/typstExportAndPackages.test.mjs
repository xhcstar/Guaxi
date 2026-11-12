// Typst 导出与包下载配置的契约测试。
//
// 这批能力是后端命令 + 前端分派两层拼起来的，最容易出的问题是「菜单里有、执行时报
// unknown command」或「后端支持、菜单里看不到」。这里把两侧的对应关系锁住。
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')

// ───────────────────────────── 后端导出命令

test('Typst 三种新导出命令都已实现并注册', () => {
  const rs = read('src-tauri/src/commands/typst_compile.rs')
  for (const fn of ['export_typ_html', 'export_typ_svg', 'export_typ_images']) {
    assert.ok(rs.includes(`pub async fn ${fn}(`), `缺少 ${fn} 实现`)
  }
  const main = read('src-tauri/src/main.rs')
  for (const fn of ['export_typ_html', 'export_typ_svg', 'export_typ_images']) {
    assert.ok(
      main.includes(`commands::typst_compile::${fn},`),
      `${fn} 没有注册到 invoke_handler（前端调用会报 unknown command）`,
    )
  }
})

test('HTML 导出走 typst 的 html 目标，而不是把 PDF 转字符串', () => {
  const rs = read('src-tauri/src/commands/typst_compile.rs')
  const start = rs.indexOf('fn compile_typ_to_html')
  assert.ok(start > 0)
  const body = rs.slice(start, start + 700)
  assert.ok(body.includes('HtmlDocument'), 'HTML 导出应编译为 HtmlDocument')
  assert.ok(body.includes('typst_html::html'), '应调用 typst_html::html 编码')
})

test('SVG 导出逐页渲染，并按页号命名输出', () => {
  const rs = read('src-tauri/src/commands/typst_compile.rs')
  // 渲染在 compile_typ_to_svgs，写盘在 export_typ_svg，两处都要检查。
  assert.ok(rs.includes('typst_svg::svg'), '应调用 typst_svg::svg')
  const start = rs.indexOf('pub async fn export_typ_svg')
  assert.ok(start > 0, '缺少 export_typ_svg')
  const body = rs.slice(start, start + 1600)
  assert.ok(body.includes('"{stem}-{}.svg"'), '应按页号生成文件名')
})

test('图片包导出是 tar.gz 归档，不是散落的文件', () => {
  const rs = read('src-tauri/src/commands/typst_compile.rs')
  const start = rs.indexOf('pub async fn export_typ_images')
  const body = rs.slice(start, start + 2000)
  assert.ok(body.includes('GzEncoder'), '应做 gzip 压缩')
  assert.ok(body.includes('tar::Builder'), '应打成 tar 归档')
})

// ───────────────────────────── 前端菜单与分派

test('Typst 菜单项与实际命令一一对应', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiEditor.vue')
  // 菜单项 key -> 后端命令
  const mapping = {
    typ_html: 'export_typ_html',
    typ_svg: 'export_typ_svg',
    typ_images: 'export_typ_images',
  }
  for (const [key, command] of Object.entries(mapping)) {
    assert.ok(vue.includes(`key: '${key}'`), `缺少菜单项 ${key}`)
    assert.ok(vue.includes(`invoke('${command}'`), `${key} 没有调用 ${command}`)
  }
})

test('导出菜单按文件类型过滤：typ 只显示它自己能产出的格式', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiEditor.vue')
  const start = vue.indexOf('const exportOptionsForActiveFile')
  assert.ok(start > 0, '缺少按文件类型过滤的导出菜单')
  const body = vue.slice(start, start + 900)
  assert.ok(body.includes("ext === 'typ'"), 'typ 应有独立分支')
  assert.ok(body.includes('TYPST_EXPORT_OPTIONS'), 'typ 应包含自己的导出项')
  assert.ok(body.includes("ext === 'tex'"), 'tex 应有独立分支（仅 PDF）')
})

test('导出前的类型校验与菜单保持一致（不会菜单里有、点了却被拒）', () => {
  const vue = read('src/components/HomeViewComponent/GuaxiEditor.vue')
  const start = vue.indexOf('const prepareExportContext')
  const body = vue.slice(start, start + 1400)
  assert.ok(body.includes('isTypstNativeFormat'), '应识别 typst 专有格式')
  assert.ok(
    body.includes("format === 'pdf' || isTypstNativeFormat"),
    'typ 文件应同时放行 pdf 与专有格式',
  )
})

// ───────────────────────────── 包下载配置

test('包下载配置支持镜像地址、缓存目录、代理与离线开关', () => {
  const rs = read('src-tauri/src/commands/typst_package.rs')
  assert.ok(rs.includes('GUAXI_TYPST_REGISTRY'), '应支持镜像环境变量')
  assert.ok(rs.includes('GUAXI_TYPST_PROXY'), '应支持代理环境变量')
  assert.ok(rs.includes('GUAXI_TYPST_PACKAGE_DIR'), '应支持缓存目录环境变量')
  assert.ok(rs.includes('GUAXI_TYPST_OFFLINE'), '应支持离线开关')
  assert.ok(rs.includes('pub fn get_typst_package_config'), '应提供读取命令')
  assert.ok(rs.includes('pub fn set_typst_package_config'), '应提供写入命令')
})

test('已缓存的包不会重复联网（第二次编译快的前提）', () => {
  const rs = read('src-tauri/src/commands/typst_package.rs')
  const start = rs.indexOf('pub fn ensure_package')
  const body = rs.slice(start, start + 1200)
  assert.ok(
    body.includes('if target.is_dir()') && body.includes('return Ok(target)'),
    '命中本地缓存时应直接返回，不应继续下载',
  )
})

test('包缓存布局与官方一致：<cache>/<ns>/<name>/<version>', () => {
  const rs = read('src-tauri/src/commands/typst_package.rs')
  const start = rs.indexOf('pub fn package_dir')
  const body = rs.slice(start, start + 400)
  assert.ok(body.includes('join(namespace)') && body.includes('join(name)') && body.includes('join(version)'))
})

test('下载地址按 <registry>/<ns>/<name>-<version>.tar.gz 拼接（镜像可直接替换）', () => {
  const rs = read('src-tauri/src/commands/typst_package.rs')
  const start = rs.indexOf('pub fn package_download_url')
  const body = rs.slice(start, start + 300)
  assert.ok(body.includes('{registry}/{namespace}/{name}-{version}.tar.gz'), body)
})

test('World 的包解析改走可配置实现，不再是写死 registry 的 PackageStorage', () => {
  const rs = read('src-tauri/src/commands/typst_compile.rs')
  assert.ok(!rs.includes('PackageStorage::new'), '不应再用写死 registry 的 PackageStorage')
  assert.ok(rs.includes('typst_package::ensure_package'), '应走可配置的包解析')
})

test('设置页提供 Typst 包下载配置入口', () => {
  const vue = read('src/components/SettingViewComponent/EditorContent.vue')
  assert.ok(vue.includes('Typst 包下载'), '设置页缺少 Typst 包下载区')
  assert.ok(vue.includes("invoke('get_typst_package_config')"), '设置页应读取配置')
  assert.ok(vue.includes("invoke('set_typst_package_config'"), '设置页应写入配置')
  // 「重启后生效」必须如实提示，否则用户会以为设置没起作用。
  assert.ok(vue.includes('重启应用后生效'), '应提示需要重启')
})


// ───────────────────────────── 构建失败日志可复制

test('构建失败面板提供一键复制（tectonic / typst 共用同一个阅读器）', () => {
  const vue = read('src/components/PdfViewer/GuaxiPdfViewer.vue')
  assert.ok(vue.includes('copyErrorLog'), '缺少复制日志的方法')
  assert.ok(vue.includes('gpv__error-copy'), '缺少复制按钮')
  assert.ok(vue.includes("writeClipboardText(props.errorText"), '复制的内容应是完整错误文本')
})

test('复制结果会回显，用户能确认是否复制成功', () => {
  const vue = read('src/components/PdfViewer/GuaxiPdfViewer.vue')
  assert.ok(vue.includes('已复制'), '成功时应提示「已复制」')
  assert.ok(vue.includes('复制失败'), '失败时应提示「复制失败」')
})

test('剪贴板写入口统一到 utils/clipboard，各组件不再各抄一份', () => {
  const util = read('src/utils/clipboard.js')
  assert.ok(util.includes('navigator.clipboard.writeText'), '应优先用 navigator.clipboard')
  assert.ok(util.includes('plugin-clipboard-manager'), '失败时应降级到 Tauri 剪贴板插件')
  // 复制失败只返回 false，不抛错——不该打断用户正在做的事。
  assert.ok(util.includes('return false'), '失败应返回 false 而不是抛错')
})
