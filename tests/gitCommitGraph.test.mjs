import test from 'node:test'
import assert from 'node:assert/strict'

import {
  computeCommitGraph,
  buildLinkPaths,
  laneColor,
  laneCenterX,
  LANE_WIDTH,
  LANE_COLOR_COUNT,
} from '../src/components/HomeViewComponent/utils/gitCommitGraph.js'

// 提交泳道图的布局计算是纯函数，可以脱离 Vue 与 Monaco 直接验证。
// 这些断言守住的是三条最容易悄悄退化的性质：
//   1. 线性历史必须只有一条 lane；
//   2. 同一条分支线在整段历史上颜色不变（曾经逐行换色，图看起来像彩虹）；
//   3. 相邻行的线在行边界处首尾相接，不会出现断口。

test('线性历史：所有提交在同一条 lane', () => {
  const { nodes, laneCount } = computeCommitGraph([
    { id: 'c', parents: ['b'] },
    { id: 'b', parents: ['a'] },
    { id: 'a', parents: [] },
  ])
  assert.equal(nodes.length, 3)
  assert.deepEqual(nodes.map((n) => n.lane), [0, 0, 0])
  assert.equal(laneCount, 1)
  assert.equal(nodes[2].outgoing.length, 0, '根提交不应再向上有输出线')
})

test('长线性历史不会让 lane 数量随行数增长', () => {
  const commits = []
  for (let i = 0; i < 300; i += 1) {
    commits.push({ id: `n${i}`, parents: i < 299 ? [`n${i + 1}`] : [] })
  }
  const { laneCount } = computeCommitGraph(commits)
  assert.equal(laneCount, 1)
})

test('合并：两个父提交分叉到两条 lane', () => {
  const { nodes, laneCount } = computeCommitGraph([
    { id: 'c', parents: ['b', 'd'] },
    { id: 'b', parents: ['a'] },
    { id: 'd', parents: ['a'] },
    { id: 'a', parents: [] },
  ])
  const byId = Object.fromEntries(nodes.map((n) => [n.id, n]))
  assert.equal(byId.c.lane, 0)
  assert.equal(byId.c.outgoing.length, 2, '合并提交应连出两条线')
  assert.equal(byId.b.lane, 0, '第一个父提交接管原 lane')
  assert.equal(byId.d.lane, 1, '第二个父提交占用新 lane')
  assert.ok(byId.a.lane <= 1, '共同祖先应复用已有 lane，而不是继续新开')
  assert.equal(laneCount, 2)
})

test('章鱼合并（多个父）不会为同一个祖先重复开 lane', () => {
  const { nodes, laneCount } = computeCommitGraph([
    { id: 'm', parents: ['x', 'y', 'z'] },
    { id: 'x', parents: ['root'] },
    { id: 'y', parents: ['root'] },
    { id: 'z', parents: ['root'] },
    { id: 'root', parents: [] },
  ])
  const byId = Object.fromEntries(nodes.map((n) => [n.id, n]))
  assert.ok(laneCount >= 3)
  assert.ok(byId.root.lane <= 3)
})

test('同一条分支线在整段历史上颜色保持不变', () => {
  const { nodes } = computeCommitGraph([
    { id: 'm3', parents: ['m2', 'f1'] },
    { id: 'm2', parents: ['m1'] },
    { id: 'f1', parents: ['m1'] },
    { id: 'm1', parents: [] },
  ])
  // 主线上 m3 → m2 → m1 应始终同色
  const mainColors = nodes
    .filter((n) => ['m3', 'm2', 'm1'].includes(n.id))
    .map((n) => n.color)
  assert.equal(new Set(mainColors).size, 1, `主线颜色应一致，实际 ${mainColors}`)
})

test('相邻行的线在行边界处相接（上一行的输出终点 = 下一行的起点）', () => {
  const { nodes } = computeCommitGraph([
    { id: 'c', parents: ['b'] },
    { id: 'b', parents: ['a'] },
    { id: 'a', parents: [] },
  ])
  for (let i = 0; i < nodes.length - 1; i += 1) {
    const outEnds = nodes[i].outgoing.map((l) => l.to).sort()
    const inStarts = [
      ...nodes[i + 1].incoming.map((l) => l.to),
      ...nodes[i + 1].through.map((l) => l.from),
    ].sort()
    assert.deepEqual(outEnds, inStarts, `第 ${i} 行与第 ${i + 1} 行连线不连续`)
  }
})

test('字段缺失或为空时不抛异常', () => {
  const r = computeCommitGraph([{ id: 'a' }, {}, { id: 'b', parents: null }])
  assert.equal(r.nodes.length, 3)
  assert.ok(r.laneCount >= 1)
  assert.deepEqual(computeCommitGraph(null).nodes, [])
})

test('SVG path：直线不留空、拐弯用贝塞尔、空输入返回空串', () => {
  assert.equal(buildLinkPaths([], 20, 'out'), '')
  assert.ok(buildLinkPaths([{ from: 0, to: 0 }], 20, 'out').includes('L'))
  assert.ok(buildLinkPaths([{ from: 0, to: 1 }], 20, 'out').includes('C'))
  assert.ok(buildLinkPaths([{ from: 1, to: 0 }], 20, 'in').includes('C'))
  // 贯穿整行
  assert.equal(buildLinkPaths([{ from: 1, to: 1 }], 20, 'through'), 'M 21 0 L 21 20')
})

test('lane 几何：中心点与色板回绕', () => {
  assert.equal(laneCenterX(0), LANE_WIDTH / 2)
  assert.equal(laneCenterX(2), LANE_WIDTH * 2 + LANE_WIDTH / 2)
  assert.equal(typeof laneColor(0), 'string')
  assert.equal(laneColor(0), laneColor(LANE_COLOR_COUNT), '色板应回绕')
  assert.equal(laneColor(-1), laneColor(LANE_COLOR_COUNT - 1), '负数索引也要安全回绕')
})
