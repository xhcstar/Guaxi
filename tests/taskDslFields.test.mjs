import test from 'node:test'
import assert from 'node:assert/strict'
import { parseTaskBlocks, serializeBlock, buildNewBlock } from '../src/utils/guaxiTaskParser.js'

const sample = `##GUAXI_TASK_START
#MANAGE TASK TYPE: life 生活
#INTERNAL ID: 11111111-1111-1111-1111-111111111111
#ID: habit-read
#TYPE: task
#KIND: habit
#DEADLINE: 2026年11月06日
#FOCUS: 2026-11-06
#CREATE TIME: 2026年11月01日 08:00:00
#COMPLETED TIME: 
#ORIGIN: 
#SEVERITY: 
#PRIORITY: 
#STATUS: TODO
#REPEAT: 每天
#PROGRESS: 
#SPENT: 45
#CHECKINS: 2026-11-05, 2026-11-06
#FIX STATUS: 
#DESCRIPTION: 读完一篇长文
#ROOT CAUSE: 
#CONDITION OF OCCURRENCE: 
#FIXED DETAILS: 
#TESTING SUGGESTIONS: 
#AFFECTED RELEASE: 
#REVIEW LINK: 
#CONTENT: 
##GUAXI_TASK_END
`

test('parses and serializes the life/study fields', () => {
  const [b] = parseTaskBlocks(sample, 'life')
  assert.equal(b.kind, 'habit')
  assert.equal(b.repeat, '每天')
  assert.equal(b.spent, 45)
  assert.equal(b.focus, '2026-11-06')
  assert.deepEqual(b.checkins, ['2026-11-05', '2026-11-06'])

  const text = serializeBlock(b)
  assert.match(text, /^#KIND: habit$/m)
  assert.match(text, /^#REPEAT: 每天$/m)
  assert.match(text, /^#SPENT: 45$/m)
  assert.match(text, /^#FOCUS: 2026-11-06$/m)
  assert.match(text, /^#CHECKINS: 2026-11-05, 2026-11-06$/m)

  const [again] = parseTaskBlocks(text, 'life')
  assert.equal(again.kind, 'habit')
  assert.equal(again.repeat, '每天')
  assert.equal(again.spent, 45)
  assert.deepEqual(again.checkins, ['2026-11-05', '2026-11-06'])
})

test('legacy blocks default to kind=task and no check-ins', () => {
  const legacy = `##GUAXI_TASK_START
#INTERNAL ID: 22222222-2222-2222-2222-222222222222
#TYPE: bug
#STATUS: TODO
#DESCRIPTION: 老块
##GUAXI_TASK_END
`
  const [b] = parseTaskBlocks(legacy, 'work')
  assert.equal(b.kind, 'task')
  assert.deepEqual(b.checkins, [])
  assert.equal(b.spent, 0)
  assert.equal(b.focus, '')
})

test('new blocks keep the shape used by capture', () => {
  const b = buildNewBlock({ category: 'study', taskType: 'task' })
  assert.equal(b.kind, 'task')
  assert.deepEqual(b.checkins, [])
  assert.equal(b.spent, 0)
})
