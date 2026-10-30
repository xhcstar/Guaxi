// 人工挑色主题的角色表。
//
// 绝大多数主题的角色色可以从上游样式表中解析出来（见 syntaxPalette.resolveRoleColors），
// 但 Monokai / Gruvbox 这类主题的 hljs 类名与语义对不上：
// Gruvbox 把标签写成青色、把函数写成绿色，Monokai 的变量和常量又共用一个橙色。
// 按类名解析会让「变量」整族跑偏，所以这两套直接给 9 个角色色。
//
// 角色语义：
//   purple = 关键字 / 宏 / 装饰器
//   green  = 字符串
//   blue   = 函数 / 方法
//   yellow = 类 / 类型 / 继承 / 命名空间
//   orange = 数字 / 布尔 / 常量 / 属性
//   red    = 变量 / 成员 / 参数
//   cyan   = 运算符 / 内置 / 转义
export const GUAXI_CODE_THEME_ROLES = {
  monokai: {
    light: {
      base: '#272822',
      muted: '#8b8874',
      purple: '#c4005c',
      green: '#5e6d00',
      blue: '#3a7a00',
      yellow: '#0f6c8a',
      orange: '#7b3fa0',
      red: '#b35c00',
      cyan: '#c4005c',
    },
    dark: {
      base: '#f8f8f2',
      muted: '#75715e',
      purple: '#f92672',
      green: '#e6db74',
      blue: '#a6e22e',
      yellow: '#66d9ef',
      orange: '#ae81ff',
      red: '#fd971f',
      cyan: '#f92672',
    },
  },
  gruvbox: {
    light: {
      base: '#3c3836',
      muted: '#928374',
      purple: '#9d0006',
      green: '#79740e',
      blue: '#427b58',
      yellow: '#b57614',
      orange: '#8f3f71',
      red: '#076678',
      cyan: '#af3a03',
    },
    dark: {
      base: '#ebdbb2',
      muted: '#928374',
      purple: '#fb4934',
      green: '#b8bb26',
      blue: '#8ec07c',
      yellow: '#fabd2f',
      orange: '#d3869b',
      red: '#83a598',
      cyan: '#fe8019',
    },
  },
}
