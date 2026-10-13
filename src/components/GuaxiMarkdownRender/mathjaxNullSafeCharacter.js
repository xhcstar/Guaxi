// MathJax 3.2.2 的一处未捕获空指针，会在公式里混入特定字符时让**整篇** Markdown 渲染失败。
//
// 问题在 mathjax-full/js/input/tex/base/BaseConfiguration.js 的 Other() 里：
//     var range = getRange(char);
//     var type  = (range ? range[3] : 'mo');                  // ← 这里判了空
//     var mo    = parser.create('token', type, def, ...);
//     range[4] && mo.attributes.set('mathvariant', range[4]);  // ← 这里漏了
// 而 getRange() 在字符不属于任何算子区间时是**明确 return null** 的（见
// core/MmlTree/OperatorDictionary.js 末尾）。于是这一行抛
// "Cannot read properties of null (reading '4')"，TeX 解析中断，整次渲染失败。
//
// 会命中的是 15 段确定的码位区间，其中真实文档里最常见的是：
//   U+0000–U+001F  ASCII 控制符（制表/换页等，复制粘贴与模型输出里常夹带）
//   U+0080–U+009F  C1 控制符（同上，且不可见，肉眼完全看不出来）
//   U+E000–U+F8FF  私用区
// 后果不是"这条公式画不出来"，而是 renderMarkdown 外层 try/catch 兜底返回原文，
// 用户看到**整篇文档变成未渲染的 Markdown 源码**。
//
// 修法上的两个要点：
// 1. 只替换 base 配置里 `fallback.character` 这一个入口，**不动 getRange 本身**。
//    core/MmlTree/MmlNodes/mo.js 也调用 getRange，那边是正确判空的（`if (range)`）；
//    如果去改 getRange 的返回值，反而会让 mo.js 走进它本来不该走的分支、改变字距。
// 2. 对 range 非 null 的字符原样转发给原函数，所以**所有原本能正常渲染的公式输出
//    逐字节不变**（已对 20 条覆盖分式/求和/矩阵/中文/花体等的公式逐一比对验证）。
//
// 本文件在模块求值时即安装补丁。TeX 输入 jax 实例是在 md.use()/首次渲染时才建的，
// 晚于本模块求值，所以补丁一定先于它生效。
import { ConfigurationHandler } from 'mathjax-full/js/input/tex/Configuration.js'
import { getRange } from 'mathjax-full/js/core/MmlTree/OperatorDictionary.js'
import { MapHandler } from 'mathjax-full/js/input/tex/MapHandler.js'
import NodeUtilModule from 'mathjax-full/js/input/tex/NodeUtil.js'
// 仅为触发副作用：Configuration.create('base', ...) 在该模块求值时执行并注册 'base' 配置。
import 'mathjax-full/js/input/tex/base/BaseConfiguration.js'

// mathjax-full 是 CJS：不同打包器对 default 的封装不一致，两种形态都兜住。
const NodeUtil = NodeUtilModule?.default || NodeUtilModule

let installed = false

/**
 * 安装"range 为空时不再崩"的字符兜底处理器。幂等，可重复调用。
 * @returns {boolean} 是否安装成功（拿不到 base 配置时返回 false，此时不改变任何行为）
 */
export const installMathJaxNullSafeCharacter = () => {
  if (installed) return true

  const configuration = ConfigurationHandler?.get?.('base')
  const originalOther = configuration?.fallback?.character
  if (typeof originalOther !== 'function') return false

  configuration.fallback.character = (parser, char) => {
    if (getRange(char) !== null) return originalOther(parser, char)

    // 与上游 null 分支**本应**产生的结果保持一致：type 取 'mo'、不设 mathvariant、
    // 并打上 fixStretchy 标记（对应上游 `if (type === 'mo')` 那一段）。
    const font = parser.stack.env['font']
    const def = font ? { mathvariant: font } : {}
    const remap = MapHandler.getMap('remap').lookup(char)
    const mo = parser.create('token', 'mo', def, remap ? remap.char : char)
    NodeUtil.setProperty(mo, 'fixStretchy', true)
    parser.configuration.addNode('fixStretchy', mo)
    return parser.Push(mo)
  }

  installed = true
  return true
}

installMathJaxNullSafeCharacter()
