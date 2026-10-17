import test from 'node:test'
import assert from 'node:assert/strict'

import { registerFilePathMutationGuard, runFilePathMutationGuards } from '../src/utils/filePathMutation.js'

test('路径变更前会依次执行已注册的守卫', async () => {
  const calls = []
  const disposeA = registerFilePathMutationGuard(async (mutation) => {
    calls.push(`a:${mutation.type}:${mutation.oldPath}`)
  })
  const disposeB = registerFilePathMutationGuard(async (mutation) => {
    await new Promise((resolve) => setTimeout(resolve, 5))
    calls.push(`b:${mutation.oldPath}:${mutation.newPath}`)
  })

  try {
    await runFilePathMutationGuards({ type: 'rename', oldPath: 'D:/a.md', newPath: 'D:/b.md' })
    assert.deepEqual(calls, ['a:rename:D:/a.md', 'b:D:/a.md:D:/b.md'])
  } finally {
    disposeA()
    disposeB()
  }
})

test('单个守卫抛错不能阻止后续守卫执行', async () => {
  const calls = []
  const disposeBad = registerFilePathMutationGuard(async () => {
    throw new Error('boom')
  })
  const disposeGood = registerFilePathMutationGuard(async (mutation) => {
    calls.push(mutation.type)
  })

  try {
    await assert.doesNotReject(runFilePathMutationGuards({ type: 'delete', oldPath: 'D:/a.md' }))
    assert.deepEqual(calls, ['delete'])
  } finally {
    disposeBad()
    disposeGood()
  }
})

test('反注册后守卫不再执行', async () => {
  let count = 0
  const dispose = registerFilePathMutationGuard(() => {
    count += 1
  })
  dispose()
  await runFilePathMutationGuards({ type: 'move', oldPath: 'D:/a.md', newPath: 'D:/b.md' })
  assert.equal(count, 0)
})
