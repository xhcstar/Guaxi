// VS Code 的 codicon 字体。
//
// GitPanel 的按钮一直写成 `codicon codicon-add` 这种类名，但项目只通过 Monaco
// 的 CSS 带进来了几个字形规则，add / discard / git-branch 这类都没有定义，
// 图标于是渲染成空盒子。这里直接复用 monaco-vscode-api 里的字体文件和全部
// 码点表，一次性生成 class 规则；字体与 VS Code 同源，新增图标不用再手抄码点。

import '@codingame/monaco-vscode-api/vscode/vs/base/browser/ui/codicons/codicon/codicon.css'
// 仅为了执行 register()，把码点表填进 codiconsUtil 的注册表。
import '@codingame/monaco-vscode-api/vscode/vs/base/common/codiconsLibrary'
import { getCodiconFontCharacters } from '@codingame/monaco-vscode-api/vscode/vs/base/common/codiconsUtil'

let injected = false

export function ensureCodiconStyles() {
  if (injected || typeof document === 'undefined') return
  injected = true

  const characters = getCodiconFontCharacters()
  const rules = Object.keys(characters)
    .map((id) => `.codicon-${id}::before{content:"\\${characters[id].toString(16)}"}`)
    .join('\n')

  const style = document.createElement('style')
  style.id = 'guaxi-codicon-glyphs'
  style.textContent = rules
  document.head.appendChild(style)
}
