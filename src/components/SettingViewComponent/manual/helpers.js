// 手册正文里的多行代码/围栏示例统一用这个 helper 拼，
// 这样章节数据里不必为了反引号再转义一层。
export const code = (...rows) => rows.join('\n')
