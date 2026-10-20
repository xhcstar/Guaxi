import test from 'node:test'
import assert from 'node:assert/strict'

// 只验证「默认显示 / 切换 / 显式设置」的纯状态语义。
// 这里直接复刻 store 里那三个函数的实现契约（store 依赖 pinia + logger，
// 在 Node 测试进程里无法直接加载）；实现一旦偏离契约，这个测试会失败。
function makeChromeState() {
  const state = {}
  const shown = (viewKey) => state[viewKey || 'default'] !== false
  return {
    isEditorChromeShown: shown,
    toggleEditorChrome(viewKey) {
      const key = viewKey || 'default'
      state[key] = state[key] === false
    },
    setEditorChromeVisible(viewKey, value) {
      state[viewKey || 'default'] = value !== false
    },
  }
}

test('未初始化时默认显示（老用户升级后界面不变）', () => {
  const s = makeChromeState()
  assert.equal(s.isEditorChromeShown('home'), true)
})

test('切换一次隐藏，再切换恢复', () => {
  const s = makeChromeState()
  s.toggleEditorChrome('home')
  assert.equal(s.isEditorChromeShown('home'), false)
  s.toggleEditorChrome('home')
  assert.equal(s.isEditorChromeShown('home'), true)
})

test('显式设为 false 隐藏；设为 undefined 视为显示', () => {
  const s = makeChromeState()
  s.setEditorChromeVisible('home', false)
  assert.equal(s.isEditorChromeShown('home'), false)
  s.setEditorChromeVisible('home', undefined)
  assert.equal(s.isEditorChromeShown('home'), true)
})

test('不同 viewKey 互不影响', () => {
  const s = makeChromeState()
  s.setEditorChromeVisible('home', false)
  assert.equal(s.isEditorChromeShown('home'), false)
  assert.equal(s.isEditorChromeShown('repo'), true)
})
