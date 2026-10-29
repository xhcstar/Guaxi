import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第六轮反馈的源码契约：
//   1. 图片 / 表格的工具不再占一整条第二行工具栏，改成贴着目标浮出的小条 + 右键菜单；
//      右键菜单不许开滚动条，也不许被编辑区的滚动容器裁掉；
//   2. 用户自己新建的文件夹不能被一次文件夹刷新抹掉；
//   3. 右键「移动」的子菜单要能看到那些自建文件夹；
//   4. 拖邮件行要真的能开始拖——Tauri 在 Windows 上默认接管原生拖放，
//      页面内的 HTML5 拖放因此整个失效，这跟前端代码无关（前两轮都在错的方向上修）。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const composer = read('src/components/ApplicationViewComponent/apps/mail/MailComposer.vue')
const css = read('src/components/ApplicationViewComponent/apps/mail/mail.css')
const mailApp = read('src/components/ApplicationViewComponent/apps/MailApp.vue')
const storeRs = read('src-tauri/src/commands/mail/store.rs')
const mod = read('src-tauri/src/commands/mail/mod.rs')
const store = read('src/stores/mailStore.js')
const conf = read('src-tauri/tauri.conf.json')

test('编辑器：图片/表格工具合并成浮动小条，不再铺第二行工具栏', () => {
  assert.ok(!/mail-compose-subbar/.test(composer), '第二行工具栏应整体删除')
  assert.ok(!/mail-compose-subbar/.test(css), '对应样式也要一起删掉，不留死样式')
  assert.ok(/const editBar = ref\(null\)/.test(composer), '要有浮动条状态')
  assert.ok(/class="mail-edit-bar"/.test(composer) && /\.mail-edit-bar \{/.test(css), '浮动条要有结构与样式')
  assert.ok(/function openEditMenu/.test(composer), '浮动条上的「更多」要能打开同一张右键菜单')
  assert.ok(/function refreshEditBar/.test(composer), '目标移动 / 滚动后浮条要跟着重算位置')
  // 浮条放不下整张表格的全部操作，剩下的必须能在右键菜单里找到（不能做了按钮没实现）。
  assert.ok(/imageMenuAction/.test(composer), '图片右键菜单要有宽度 / 对齐 / 删除动作')
  assert.ok(/editorMenu\.value = \{ \.\.\.spot, image: true \}/.test(composer), '右键点图片要开图片分支')
})

test('编辑器右键菜单：不滚动、不被裁、放得下所有条目', () => {
  const menuBlock = css.slice(css.indexOf('.mail-editor-menu {'), css.indexOf('.mail-editor-menu-head'))
  assert.ok(/position: fixed/.test(menuBlock), '菜单要 fixed，脱离编辑区的滚动容器')
  assert.ok(!/max-height|overflow-y/.test(menuBlock), '不许靠滚动条装内容')
  assert.ok(/\.mail-editor-menu\.is-columns/.test(css) && /mail-editor-menu-col/.test(composer), '表格菜单两栏排版才放得下')
  assert.ok(/function clampEditorMenu[\s\S]{0,900}window\.innerHeight/.test(composer), '超出视口要夹回来')
  // 主题变量（--mail-*）挂在邮件根节点上：菜单一旦被传送到 body，var() 全部落空，
  // 背景变成透明、字也看不清（真实反馈）。fixed 定位不传送到 body 也不会被裁。
  assert.ok(!/<Teleport/.test(composer), '浮层必须留在邮件根节点内，否则拿不到主题变量')
  assert.ok(/position: fixed/.test(css.slice(css.indexOf('.mail-edit-bar {'), css.indexOf('.mail-edit-bar-label'))), '浮条要 fixed 定位')
})

test('文件夹：用户自建的目录不能被一次刷新抹掉', () => {
  // 服务器 LIST 里还没有它（刚建好、或这台服务器不返回）是常态：
  // 只保 local_only 的话，下一次刷新就把用户刚建的文件夹删了。
  assert.ok(
    /FROM folders WHERE account_id=\?1 AND \(local_only=1 OR user_created=1\)/.test(storeRs),
    '保留条件要带上 user_created',
  )
  assert.ok(/let server_names: std::collections::HashSet/.test(storeRs), '服务器返回的要覆盖本地那一份')
  assert.ok(/!server_names\.contains\(f\.name\.as_str\(\)\)/.test(storeRs), '只补服务器没返回的')
})

test('移动子菜单：目标清单要包含自建文件夹', () => {
  assert.ok(/const contextFolders = computed/.test(mailApp), '子菜单要读实时文件夹表')
  assert.ok(/FOLDER_ROLE_ORDER = \[[^\]]*'custom'/.test(mailApp), '自建文件夹要排在角色序里，不能被过滤掉')
  assert.ok(
    /selectableMoveTargets\(store\.foldersByAccount/.test(mailApp),
    '目标清单走共享规则：排除发件箱与模板壳，其余都可归档',
  )
})

test('文件夹：刷新要返回落库后的完整表，否则侧栏会在两次请求之间来回抖', () => {
  // 真实反馈：新建的文件夹「几秒钟后消失，过一会儿又出现」。
  // 原因是刷新路径只把服务器/固定那份返回给前端，而读库路径带着自建夹：
  // 两条路径交替写 foldersByAccount，侧栏就跟着抖。
  const pop3 = read('src-tauri/src/commands/mail/pop3_sync.rs')
  assert.ok(
    /store::replace_folders\(&account\.id, &folders\)\?;\s*\n\s*store::list_folders\(&account\.id\)\s*\n\}/.test(
      pop3,
    ),
    'POP3 的 ensure_folders 要返回落库后的完整表',
  )
  assert.ok(
    /store::replace_folders\(&account\.id, &folders\)\?;[\s\S]{0,200}store::list_folders\(&account\.id\)\s*\n\}/.test(
      mod,
    ),
    'IMAP 刷新也要返回落库后的完整表',
  )
  // 前端：慢的那次请求后到不能把新列表盖回去。
  assert.ok(/const foldersRequestSeq = new Map\(\)/.test(store), '文件夹请求要带序号')
  assert.ok(/if \(foldersRequestSeq\.get\(accountId\) !== seq\) return/.test(store), '旧请求的结果要丢弃')
})

test('编辑器右键菜单：字号收进下拉，菜单颜色跟随主题', () => {
  assert.ok(/mail-editor-menu-select/.test(composer), '字号改成下拉')
  assert.ok(!/v-for="size in CELL_FONT_SIZES"[\s\S]{0,120}mail-editor-menu-chip/.test(composer), '不再一个个平铺字号')
  // 主题变量挂在邮件根节点上，浮层不能离开那棵子树。
  assert.ok(!/<Teleport/.test(composer), '不能把菜单传送到 body（会丢主题变量、背景变透明）')
})

test('拖拽：关掉 Tauri 的原生拖放接管，页面内的 HTML5 拖放才会生效（OCR / 附件落点仍需要）', () => {
  // Windows 上 dragDropEnabled 默认为 true：webview 自己注册了 OLE 放置目标，
  // 页面上的 dragstart/drop 根本收不到。
  assert.ok(/"dragDropEnabled": false/.test(conf), '主窗口要关掉原生拖放接管')
})
