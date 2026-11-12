# 阅读位置：保存、精确定位与恢复

这份文档说明 `GuaxiEditor.vue` 里「阅读位置」这一整套机制**现在**由哪些模块组成、
各自负责什么、规则为什么这么定，以及改动时该去哪里验证。

改这块之前请先读这里：这套逻辑历史上出过好几轮「切回来回到开头」「只能回到大概位置」
「重启记不住」的 bug，而它们全都只在特定操作顺序 + 特定渲染节奏下才复现。

---

## 一、模块划分

| 文件 | 职责 | 依赖 |
| --- | --- | --- |
| `readPositionCore.js` | 纯决策：路径归一、缓存 key、载体选择、**目标换算（锚点/像素/比例）**、**落盘序列化结构** | 无 |
| `readPositionStore.js` | 落盘：序列化 → localStorage，防抖写入、读回、脏数据降级 | `readPositionCore` |
| `readPositionRestore.js` | 逐帧落位引擎 + 落位后的短暂锚点看护 | `readPositionCore` |
| `GuaxiEditor.vue` | 接线：什么时候存、什么时候恢复、DOM 读数 | 以上三者 |

三个独立模块都**不依赖 Vue / Monaco / Tauri**，因此可以在 Node 里直接测，
也可以在浏览器里对真实滚动容器跑（`.readpos-preview/`）。

`GuaxiEditor.vue` 里剩下的只有「读 DOM、决定时机、调模块」这部分——那是唯一必须和
组件生命周期绑在一起的部分。

---

## 二、五种载体

「阅读位置」不是一个数，而是按**模式**分到不同载体上的记录：

| 模式 | 载体 | 记录什么 |
| --- | --- | --- |
| `edit` | Monaco editor | viewState（光标/选区/滚动）+ 兜底 scrollTop |
| `preview` / `split` | Markdown 预览容器 | scrollTop + 比例 + **内容锚点** |
| `preview`（.tex/.typ） | PDF 阅读器 | 比例 + **页号 + 页内 pt** |
| `preview`（.org） | 任务预览面板 | 比例 + 卡片锚点（面板自己实现） |
| `explorer` | notebook 滚动容器 | scrollTop + cellKey |
| `realtime` | Vditor 内容区 | scrollTop + 比例 |

缓存 key 是 `kind:path`。同一文件在编辑与预览下存两份，互不覆盖——
这是「同一文档切模式」能各自记住位置的基础。

注意两个「看起来该分开、实际必须共用」的组合：`edit` 与 `split` 共用 Monaco 那台编辑器，
`split` 的预览侧与 `preview` 共用同一个预览容器。它们物理上是同一个 DOM 元素，
分成两份记录反而会互相打架。真正必须隔离的是「编辑器 vs 预览」（两个 DOM）与
「探索者 vs 其它」（探索者是另一个挂载点的组件）。

---

## 三、精确坐标：为什么是「行号 + 块内偏移」

像素和比例都依赖文档高度：

- **像素**只在文档高度与记录时完全一致时才指向同一处；
- **比例**在图片、公式、Mermaid 图表加载前后换算出的位置会漂。

Markdown 渲染器给每个块写了 `data-md-start-line`（源码行号）。行号在任何窗口宽度、
字体、图片加载状态下都指向同一段文字，所以它是唯一真正稳定的坐标。

**符号约定（贯穿 core / 引擎 / 组件，改一处必须三处一致）：**

```
anchorOffset = 视口顶部 - 块顶        // 即「块被视口截掉了多少」
恢复时 scrollTop = 块在文档中的顶部 + anchorOffset
```

反过来写（`块顶 - 视口顶部`）会让恢复差整整一个块的高度。
这个符号错误在浏览器验证里被真实抓到过一次（`drift = 156px ≈ 一个块的高度`）。

恢复时的优先级：**锚点 > 像素（仅当 `isPixelTrustworthy`） > 比例**。

---

## 四、逐帧落位与「看护」

预览是渐进渲染：图片解码、KaTeX、Mermaid 会在此后若干帧继续把文档撑高。所以：

1. **落位不能只写一次** `scrollTop` —— 会被当时偏小的 `scrollHeight` 夹断；
2. **不能把「这一帧算不出目标」当成「目标就是 0」** —— 那正是「回到开头」；
3. **不能停太晚** —— 会和用户抢滚动条。

收敛规则（`isRestoreSettled`）：目标为 0 立即结束；否则必须等可滚动高度连续
`RESTORE_SCROLL_STABLE_FRAMES` 帧不变。记录里有目标、这一帧却算不出来时传
`awaitingTarget: true`，继续等（在重试预算内）。

落位**收工之后**还有一个短暂看护（`keepAnchorStableAfterRestore`）：图片/图表常常在
「文档看起来稳定」之后才真正改变高度，只要它们出现在视口上方，用户正看的那一段就被推走。
看护每帧核对锚点并纠回，**用户一旦滚动/点击/按键立刻放手**。

---

## 五、时机与静音

保存时机：各处滚动防抖（180ms）、切模式前、切 tab 前、切文件前、主页 `onDeactivated`、
组件 `onUnmounted`。

恢复时机：切文件后（且**在模式恢复之后**）、切模式后、`onActivated`、预览/PDF/org
渲染完成后消费恢复队列。

两条必须遵守的约定：

1. **未参与布局时不保存。** 元素被 `v-show` 隐藏或被 KeepAlive 移出文档时
   `scrollTop` 恒为 0，此时保存会把正确位置覆盖成开头。各载体都有这条判断。
2. **落位过程静音保存。** 用**计数**（`suppressPreviewReadPositionSaveDepth`）而不是布尔：
   两次落位可能重叠，布尔会被先结束的那次提前关掉。

---

## 六、恢复队列

`pendingPreviewReadPositionRestoreKeys` 是一个**按 key 去重的小队列**（上限 12），
不是单个字符串槽位。原因：切文件的一瞬间会连续排队多个请求（分屏两侧、模式切换前后
两轮补落位、PDF 构建完成回填），单槽位会让后来的覆盖前面的，表现为「这一次切回来没落位」。

切到**不同文件**时要 `clearPendingPreviewReadPositionRestores()`：否则 A 的请求会在
B 渲染完成时被消费。

---

## 七、持久化（跨重启）

`readPositionStore.js` 把五类缓存 + 每文件模式记忆 + 最后打开的文档写进
`localStorage['guaxi_editor_read_position_v1']`。

- **版本号**：结构不符整体丢弃，不尝试猜旧数据。
- **防抖 400ms**；`onDeactivated` / `onUnmounted` 时立刻 flush（那是最可能关窗的时刻）。
- **任何写失败静默**：隐私模式、配额满都只是这一次记不住，不影响使用。
- 读回时只填缓存，**不直接落位**——恢复仍走各模式原有链路，避免出现两套行为。

开机流程：`readPositionPersistence.load()`（填缓存 + 模式记忆）
→ `restoreHomePreviewMode('mounted')`
→ `maybeReopenLastDocument()`（只在启动后没有任何标签页时才重开，避免和外部打开抢）。

> 已知限制：Monaco 的 `viewState` 不可序列化，重启后编辑器侧只能按 scrollTop 近似落位；
> 预览侧与 PDF 侧有锚点，可以精确恢复。

---

## 八、验证

```bash
node --test tests/readPosition*.test.mjs tests/editorReadPosition.test.mjs
```

| 测试文件 | 覆盖 |
| --- | --- |
| `editorReadPosition.test.mjs` | 载体与 key 选择、跨页面往返 |
| `readPositionPrecision.test.mjs` | 锚点、像素可信度、收敛规则、落盘结构 |
| `readPositionRestoreEngine.test.mjs` | 逐帧落位（渐进渲染）、看护与放手 |
| `readPositionSwitching.test.mjs` | 150 种「文档 × 模式」切换矩阵、重启往返 |
| `readPositionReopen.test.mjs` | 重开上次文档 |

浏览器端（真实滚动容器，验证时序相关的行为）：

```bash
npx vite --config .readpos-preview/vite.config.mjs   # 端口 8941
```

打开后自动跑完并把结果打在右侧。它用的是**线上同一份** `readPositionCore` /
`readPositionStore` / `readPositionRestore`。

---

## 九、改动前的检查清单

- [ ] 动了 `anchorOffset` 的符号？core、引擎、组件、PDF 侧四处一起改，并跑浏览器验证。
- [ ] 新增载体？在 `collectPersistedReadPositions` 与 `restorePersistedReadPositions` 都登记。
- [ ] 新增恢复入口？确认它排在**模式恢复之后**，且不会绕过「未参与布局不保存」的约定。
- [ ] 动了静音逻辑？确认仍用**计数**，且 `finally` 里一定递减。
- [ ] 动了预览渲染完成消费队列？确认空渲染阶段不消费（`!renderedHtml.value` 分支）。
