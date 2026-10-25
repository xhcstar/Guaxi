import test from 'node:test'
import assert from 'node:assert/strict'

import { buildSpeakerOptions, describeSpeaker, speakerMeta } from '../src/utils/ttsSpeakers.js'

const KOKORO = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
const VITS = 'utils/Models/tts/vits-melo-tts-zh-en/model.onnx'

test('中文男声筛选只返回 zm_ 音色', () => {
  const groups = buildSpeakerOptions(KOKORO, { language: 'zh', gender: 'male' })
  assert.equal(groups.length, 1)
  assert.equal(groups[0].label, '中文男声')
  assert.ok(groups[0].children.every((option) => /zm_/.test(option.label)))
  assert.ok(groups[0].children.some((option) => option.value === '65'))
})

test('英文筛选只返回英文音色', () => {
  const groups = buildSpeakerOptions(KOKORO, { language: 'en', gender: 'auto' })
  const values = groups.flatMap((group) => group.children.map((option) => option.value))
  assert.deepEqual(values.sort(), ['0', '1', '2'])
})

test('默认不过滤时保留所有分组', () => {
  const groups = buildSpeakerOptions(KOKORO)
  assert.ok(groups.length >= 4)
  assert.ok(groups.every((group) => group.children.length > 0))
})

test('非 Kokoro 模型忽略筛选，仍按编号给出音色', () => {
  const options = buildSpeakerOptions(VITS, { language: 'zh', gender: 'male' })
  assert.equal(options.length, 16)
  assert.equal(options[0].label, 'Speaker 0')
})

test('speakerMeta 解析语言与性别', () => {
  assert.deepEqual(speakerMeta(KOKORO, '65'), { language: 'zh', gender: 'male', name: 'zm_016' })
  assert.deepEqual(speakerMeta(KOKORO, '0'), { language: 'en', gender: 'female', name: 'af_maple' })
  assert.equal(speakerMeta(KOKORO, '999').name, '')
})

test('describeSpeaker 给出可读的中文说明', () => {
  const text = describeSpeaker(KOKORO, '65')
  assert.match(text, /中文男声/)
  assert.match(text, /zm_016/)
  assert.match(text, /65/)
})
