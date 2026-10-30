// MathJax 3 的 TeX 输入不含 LaTeX 的 bm 宏包，也没有 RR / dd 这类常用别名。
// 模型或论文里写 bm{D}、RR^2 时，mathjax3 的 noundefined 扩展会把未知命令画成
// 红色错误文本（截图里公式中夹的红色命令就是这种）。这里集中补一组等价别名，
// 纯增量配置：原本能解析的公式输出逐字节不变。
//
// 主线程（GuaxiMarker）与公式渲染 Worker 共用这一份，避免两边宏表漂移。
//
// 反斜杠统一用 String.fromCharCode(92) 拼，避免源码里出现转义歧义。
const r = (latex) => String.fromCharCode(92) + latex

export const GUAXI_TEX_MACROS = {
  // LaTeX bm 宏包：\bm{...} 等价于 \boldsymbol{...}；也兼容 \bm D 这种单 token 写法。
  bm: [r('boldsymbol{#1}'), 1],
  RR: [r('mathbb{R}'), 0],
  NN: [r('mathbb{N}'), 0],
  ZZ: [r('mathbb{Z}'), 0],
  QQ: [r('mathbb{Q}'), 0],
  CC: [r('mathbb{C}'), 0],
  // 物理/工程笔记里常见的微分 d 正体。
  dd: [r('mathrm{d}'), 0],
}
