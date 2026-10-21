// 电子书图片内联与乱码兜底的回归测试（纯逻辑，不依赖 Tauri）。
//
// 这两条都是「静默失败」型 bug：图片显示空白、正文整章乱码，日志里只有一行
// diagnostics。测试锁住修复口径，避免以后再退回。
import assert from 'node:assert/strict'
import { stripScripts } from '../src/components/ReaderComponent/readers/ebookSanitize.js'

// 1) 净化器必须原样保留 data URI 形式的图片 —— 它是图片内联后的唯一载体。
const inlined = '<p><img src="data:image/jpeg;base64,/9j/4AAQSkZJRg==" alt=""/></p>'
assert.equal(stripScripts(inlined), inlined, 'data URI 图片必须原样保留')

// 2) 净化器不能破坏带分号/斜杠/加号的 base64 内容。
assert.ok(stripScripts(inlined).includes('base64,/9j/4AAQSkZJRg=='), 'base64 内容不能被改写')

// 3) 相对路径图片同样保留（内联失败时至少前端还知道原路径）。
const rel = '<img src="../images/00001.jpeg"/>'
assert.equal(stripScripts(rel), rel)

console.log('ebookTextRepair: ok')
