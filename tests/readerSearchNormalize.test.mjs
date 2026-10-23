import test from 'node:test'
import assert from 'node:assert/strict'

import { normalizeWithMap } from '../src/components/ReaderComponent/readerSearch.js'

// 回归测试：搜索的「规范化串偏移」必须能映射回原始文本的下标。
//
// 背景（真实问题）：阅读器里搜中文，高亮却落在旁边几个字上。
// 根因是匹配在 `textContent.toLowerCase()` 上做、切片却回到原始文本，
// 一旦正文含全角标点、不换行空格或软连字符，两套下标就对不上。
// 这里锁住三件事：规范化结果、映射的正确性、以及幂等性（高亮时依赖它）。

const sliceByMap = (raw, start, end) => {
  const { map } = normalizeWithMap(raw)
  return raw.slice(map[start], map[end - 1] + 1)
}

test('全角 ASCII 折半角，且长度不变', () => {
  const raw = 'ＡＢＣ　１２３'
  const { text, map } = normalizeWithMap(raw)
  assert.equal(text, 'abc 123')
  assert.equal(map.length, [...text].length, 'map 与规范化串等长')
})

test('连续空白折叠成一个空格，映射指向这段空白的首字符', () => {
  const raw = '甲  　 乙'
  const { text, map } = normalizeWithMap(raw)
  assert.equal(text, '甲 乙')
  // 规范化串第 1 个字符是那个空格，它对应 raw 里的第一个不换行空格（index 1）。
  assert.equal(map[1], 1)
  assert.equal(raw[map[2]], '乙')
})

test('零宽字符被删除，不占位', () => {
  const raw = '拜​占­庭﻿'
  const { text } = normalizeWithMap(raw)
  assert.equal(text, '拜占庭')
})

test('映射能还原任意子串（这是「搜到哪、高亮到哪」的正确性前提）', () => {
  const raw = '第 4 章　所有权（Ownership）与借用'
  const { text } = normalizeWithMap(raw)
  // 查询词同样要过一遍规范化：读者输入半角括号，正文里是全角，两者必须对上。
  const needle = normalizeWithMap('所有权(Ownership)').text
  const at = text.indexOf(needle)
  assert.ok(at >= 0, `应能在规范化串里找到：${text}`)
  assert.equal(
    normalizeWithMap(sliceByMap(raw, at, at + needle.length)).text,
    needle,
    '映射回原文再规范化，应还原成同一个命中词',
  )
})

test('规范化幂等：normalize(normalize(x)) === normalize(x)', () => {
  const raw = 'ａ　ｂ ｃ​'
  const once = normalizeWithMap(raw).text
  assert.equal(normalizeWithMap(once).text, once)
})

test('大小写折叠不破坏映射长度', () => {
  const raw = 'Hello World'
  const { text, map } = normalizeWithMap(raw)
  assert.equal(text, 'hello world')
  assert.equal(map.length, raw.length)
})
