/**
 * 提交历史泳道图（commit graph）的布局计算。
 *
 * 不依赖任何渲染库：一次线性扫描给每个提交分配 lane（竖列），并记录这一行需要画的线。
 * 渲染层只按这些坐标拼 SVG path。
 *
 * 算法（`git log --graph` 的简化版）：
 * 维护 `lanes`，每个槽位存放「这条线正在等待的提交 id」。逐行处理提交 c：
 *   1. c 所在的 lane = lanes 里第一个等于 c.id 的槽位；没有就新开一条。
 *   2. c 的第一个父提交接管这条 lane；其余父提交（合并）复用已有槽位，否则新开。
 *   3. 本行画：从上边界下来的「输入线」→ 提交圆点 → 行内横移 → 从下边界出去的「输出线」。
 *
 * 所有函数都是纯函数，可以脱离 Vue 单独验证。
 */

/** 单条 lane 的横向间距（px）。 */
export const LANE_WIDTH = 14

/** 一个 lane 槽位：正在等待的提交 id。null 表示空闲。 */
const EMPTY = null

/** 找空槽；全满就追加。 */
function takeFreeLane(lanes) {
  const idx = lanes.findIndex((v) => v === EMPTY)
  if (idx >= 0) return idx
  lanes.push(EMPTY)
  return lanes.length - 1
}

/**
 * @typedef {Object} GraphLink 本行内的一根连线
 * @property {number} from 起点 lane
 * @property {number} to   终点 lane
 * @property {number} color 配色索引
 *
 * @typedef {Object} GraphNode
 * @property {string} id
 * @property {number} lane    提交圆点所在的 lane
 * @property {number} index   行号
 * @property {number} color   圆点配色
 * @property {GraphLink[]} incoming 从上一行进入本行的线（画在圆点之上）
 * @property {GraphLink[]} outgoing 从本行出去到下一行的线
 * @property {GraphLink[]} through  只穿过本行、不与圆点相接的线
 */

/**
 * @param {Array<{id:string, parents?:string[]}>} commits 子在前、父在后
 * @returns {{ nodes: GraphNode[], laneCount: number }}
 */
export function computeCommitGraph(commits) {
  const list = Array.isArray(commits) ? commits : []
  /** @type {Array<string|null>} 每个 lane 正在等待的提交 id */
  const lanes = []
  /**
   * 提交 id → 通往它的那条线的配色。
   *
   * 颜色必须跟着**线**走，而不是跟着提交走：一条分支从分叉到合并是一根连续的线，
   * 逐行换色会让图看起来像彩虹，这也是"图很丑"最直接的来源。
   * 第一个父提交继承当前线的颜色，合并带来的新线才领一个新颜色。
   */
  const lineColor = new Map()
  let colorCursor = 0

  /** 领一个新颜色，优先挑当前没有线在用的那个，避免相邻线同色。 */
  const takeColor = () => {
    const inUse = new Set(lineColor.values())
    for (let i = 0; i < LANE_COLOR_COUNT; i += 1) {
      const candidate = (colorCursor + i) % LANE_COLOR_COUNT
      if (!inUse.has(candidate)) {
        colorCursor = (candidate + 1) % LANE_COLOR_COUNT
        return candidate
      }
    }
    const fallback = colorCursor % LANE_COLOR_COUNT
    colorCursor = (colorCursor + 1) % LANE_COLOR_COUNT
    return fallback
  }

  const nodes = []
  let maxLane = 0

  list.forEach((commit, index) => {
    const id = String(commit?.id || '')
    const parents = (Array.isArray(commit?.parents) ? commit.parents : [])
      .map((p) => String(p || ''))
      .filter(Boolean)

    // 进入本行时，各 lane 的「等待目标」就是输入线的来源。
    const lanesBefore = lanes.slice()

    // 1. 本行提交所在的 lane
    let lane = lanes.indexOf(id)
    if (lane < 0) {
      lane = takeFreeLane(lanes)
      lanes[lane] = id
    }

    // 2. 本行的颜色：桌上的小圆点用"通往它的那条线"的颜色
    if (!lineColor.has(id)) lineColor.set(id, takeColor())
    const color = lineColor.get(id)

    // 3. 输入端
    const incoming = []
    const through = []
    lanesBefore.forEach((waitingId, laneIndex) => {
      if (!waitingId) return
      if (laneIndex === lane) {
        incoming.push({ from: laneIndex, to: lane, color })
      } else {
        through.push({
          from: laneIndex,
          to: laneIndex,
          color: lineColor.get(waitingId) ?? color,
        })
      }
    })

    // 4. 输出端：父提交接管 lane
    const outgoing = []
    lanes[lane] = EMPTY
    if (parents.length === 0) {
      // 根提交：本线到此结束，颜色回收（下次领色时才会被复用）
      lineColor.delete(id)
    } else {
      parents.forEach((parentId, parentIndex) => {
        const existing = lanes.indexOf(parentId)
        if (existing >= 0) {
          // 已经有线在等这个父提交：从本行横向并过去，颜色用本行的
          outgoing.push({ from: lane, to: existing, color })
        } else if (parentIndex === 0) {
          // 第一个父提交就地接管本 lane：颜色沿这根线继续
          lanes[lane] = parentId
          lineColor.set(parentId, color)
          outgoing.push({ from: lane, to: lane, color })
        } else {
          // 合并带入的新线：占空槽、领新颜色
          const slot = takeFreeLane(lanes)
          lanes[slot] = parentId
          lineColor.set(parentId, takeColor())
          outgoing.push({ from: lane, to: slot, color })
        }
      })
    }

    maxLane = Math.max(
      maxLane,
      lane,
      ...outgoing.map((l) => l.to),
      ...through.map((l) => l.to),
    )
    nodes.push({ id, lane, index, color, incoming, outgoing, through })

    // 清掉行尾的空槽，避免 lane 数量随历史无限增长
    while (lanes.length && lanes[lanes.length - 1] === EMPTY) {
      lanes.pop()
    }
  })

  // 至少留一条 lane 的宽度，否则列表会被挤到最左边、图标贴边。
  return { nodes, laneCount: Math.max(1, maxLane + 1) }
}

/**
 * lane 配色。
 *
 * 取自 Git 图形界面的通行色板：深浅主题下都能和背景拉开，且不与
 * 「新增绿 / 删除红」的 diff 语义撞色。渲染层直接当 stroke 用，不必再维护 CSS 变量。
 */
const LANE_PALETTE = [
  '#4a9eff', // 蓝
  '#e8a33d', // 橙
  '#7bd88f', // 绿
  '#d97ad9', // 紫
  '#e06c75', // 红
  '#56c8d8', // 青
  '#c3a6ff', // 淡紫
  '#d8bd6a', // 土黄
]

export function laneColor(index) {
  const i = Math.trunc(Number(index) || 0)
  return LANE_PALETTE[((i % LANE_PALETTE.length) + LANE_PALETTE.length) % LANE_PALETTE.length]
}

/** 色板长度：领颜色时用它做回绕。 */
export const LANE_COLOR_COUNT = LANE_PALETTE.length

/** lane 索引 → 像素中心 x。 */
export function laneCenterX(lane) {
  return lane * LANE_WIDTH + LANE_WIDTH / 2
}

/**
 * 把一行的连线转成 SVG path 的 `d`。
 *
 * 用三次贝塞尔让拐弯圆润：直角的折线在密集的图上像电路板，而 Git 图形界面的观感
 * 基本就靠这个弧度。
 *
 * @param {{from:number,to:number}[]} links
 * @param {number} rowHeight
 * @param {'in'|'out'|'through'} kind 决定线段落在行的上半还是下半
 */
export function buildLinkPaths(links, rowHeight, kind) {
  const list = Array.isArray(links) ? links : []
  const mid = rowHeight / 2
  const segments = []

  for (const link of list) {
    const x1 = laneCenterX(link.from)
    const x2 = laneCenterX(link.to)

    if (kind === 'through') {
      // 整行贯穿
      segments.push(`M ${x1} 0 L ${x1} ${rowHeight}`)
      continue
    }

    if (kind === 'in') {
      // 从行上边界进入，竖到中线；若需横移，在中线上拐
      if (x1 === x2) {
        segments.push(`M ${x1} 0 L ${x1} ${mid}`)
      } else {
        const ctrl = Math.max(3, Math.abs(x2 - x1) / 2)
        segments.push(
          `M ${x1} 0 C ${x1} ${mid - ctrl}, ${x2} ${mid - ctrl}, ${x2} ${mid}`,
        )
      }
      continue
    }

    // out：从中线出发落到下边界
    if (x1 === x2) {
      segments.push(`M ${x1} ${mid} L ${x1} ${rowHeight}`)
    } else {
      const ctrl = Math.max(3, Math.abs(x2 - x1) / 2)
      segments.push(
        `M ${x1} ${mid} C ${x1} ${mid + ctrl}, ${x2} ${mid + ctrl}, ${x2} ${rowHeight}`,
      )
    }
  }

  return segments.join(' ')
}
