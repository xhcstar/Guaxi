# 数学块（命令式数学工作区）使用说明

## 它是什么

笔记本里的「数学块」是一个**命令式数学工作区（数学 REPL）**：你在里面一行一行地写
赋值、表达式或命令，点「运行」后，整段会交给后台的真实 Python 引擎
（`src-tauri/scripts/guaximath_engine.py`，基于 SymPy / NumPy / SciPy / Plotly）逐行计算，
并把结果渲染回来：

- **符号结果**（公式、解、矩阵）→ 以 LaTeX 公式显示（App 内置 MathJax），每条结果带中文标题。
- **图像**（曲线 / 曲面 / 分布）→ 以**交互式 Plotly 图**显示，可缩放、旋转、悬停查看数值。

它与「Python Cell」共享同一个持久会话：

- 数学块里 `f = sin(x)/x` 定义的符号，会**跨多次运行、跨多个数学块**持久复用。
- 也能读取前面 Python Cell 里准备好的变量。

> 需要写任意 Python 脚本时请用 Python Cell；数学块聚焦数学计算，禁用了 `import`、文件、网络等操作。

## 快速上手

新建一个数学块，输入下面内容，点工具栏的 ▶ 运行：

```
f = sin(x)/x
limit(f, x, 0)
diff(f, x)
integrate(x**2, x)
plot(f, (x, -15, 15))
```

你会看到：`f` 的定义、极限 `1`、导数、不定积分 `x³/3`，以及一张可交互的曲线图。

工具栏的 **Σ▾** 菜单可一键把常用命令模板插入到内容末尾。

### 预览模式（公式 / 命令）

输入区下方有一个**实时预览**，工具栏提供一个图标在两种预览模式间切换（默认「公式」）：

- **公式模式（默认，Σ 图标）**：把输入按 LaTeX 公式渲染，适合一边写公式一边看渲染效果。
- **命令模式（终端图标）**：把输入当作命令式工作区程序，按代码块展示源码、**不**按 LaTeX 渲染；
  适合写 `plot(...)`、`A = matrix(...)` 这类多行命令。

无论哪种预览模式，点「运行」后的**输出**都一样：符号结果走 MathJax、图走交互式 Plotly。

> 说明：交互式 Plotly 图在独立 iframe 内渲染，固定使用**亮色**主题以保证绘图清晰；
> 符号 / 文字结果走 markdown，会自动跟随 App 的明暗主题。

## 书写规则

- **赋值**：`A = matrix([[2,1],[1,2]])`、`f = exp(-x**2)`。
- **裸表达式 / 命令**：单独成行即会显示其结果，如 `det(A)`、`solve(x**2-1)`。
- **未定义的名字自动当作符号**：直接写 `x`、`y`、`theta` 即可，无需先声明；
  被当作函数调用的未知名字（如 `y(x)`）会当作未知函数，便于写微分方程。
- **幂**：`x**2`（也支持 `x^2`）。**乘号**建议显式写 `*`；数学写法 `2x`、`3sin(x)` 也能识别。
- **常量 / 函数**：`pi`、`E`、`I`、`oo`（无穷）、`sin/cos/exp/log/sqrt/Abs` 等。
- **LaTeX**：用 `tex(r"\frac{a}{b}")` 把 LaTeX 解析成表达式；
  旧版纯 LaTeX 内容（如 `\frac{\partial u}{\partial t}`）也能被兼容解析。
- 某一行写错**不会影响其它行**，只会在该行位置给出中文提示。

## 命令速查（阶段一：高数 / 线代 / 概率统计）

### 微积分
| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `limit(f, x, a)` | `limit(sin(x)/x, x, 0)` | 极限；可加 `dir="+"`/`"-"` 单侧 |
| `diff(f, x[, n])` | `diff(sin(x)*exp(x), x, 2)` | 求导（n 阶） |
| `integrate(f, x)` | `integrate(x**2, x)` | 不定积分 |
| `integrate(f, (x,a,b))` | `integrate(exp(-x**2), (x,-oo,oo))` | 定积分 |
| `series/taylor(f, x, x0, n)` | `taylor(cos(x), x, 0, 8)` | 幂级数 / 泰勒展开 |
| `solve(eq[, vars=...])` | `solve(x**2 - 5*x + 6)` | 解方程 / 方程组 |
| `dsolve(Eq(...))` | `dsolve(Eq(Derivative(y(x),x,x)+y(x),0))` | 微分方程 |
| `summation / product` | `summation(1/k**2, k, 1, oo)` | 求和 / 连乘积 |
| `simplify/expand/factor` | `factor(x**2-1)` | 化简 / 展开 / 因式分解 |

### 线性代数
| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `matrix([[...],[...]])` | `A = matrix([[2,1],[1,2]])` | 定义矩阵 |
| `det / inv / transpose` | `det(A)`、`inv(A)` | 行列式 / 逆 / 转置 |
| `rref / rank / nullspace` | `rref(A)` | 行最简形 / 秩 / 零空间 |
| `eig(A)` | `eig(A)` | 特征值与特征向量 |
| `eigenvalues / charpoly` | `charpoly(A)` | 特征值 / 特征多项式 |
| `lu / qr / svd` | `svd(A)` | 矩阵分解（SVD 为数值） |
| `linsolve(A, b)` | `linsolve(matrix([[1,1],[1,-1]]), matrix([3,1]))` | 解线性方程组 |

### 概率统计
| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `Normal(μ, σ)` | `Normal(0, 1)` | 正态分布，画 PDF/CDF + 期望方差 |
| `Uniform/Exponential/Gamma/Beta` | `Exponential(scale=2)` | 常见连续分布 |
| `Binomial/Poisson/Bernoulli/Geometric` | `Binomial(10, 0.4)` | 常见离散分布（画 PMF/CDF） |
| `expectation / variance` | `expectation(Normal(0,1))` | 期望 / 方差 |

> 分布功能依赖 `scipy`；若运行时未安装会给出中文提示。

### 绘图（交互式 Plotly）
| 写法 | 效果 |
| --- | --- |
| `plot(sin(x), (x,-10,10))` | 一维曲线（复值自动分实部/虚部） |
| `plot([sin(x), cos(x)], (x,-pi,pi))` | 多条曲线对比 |
| `plot(x**2+y**2, (x,-3,3), (y,-3,3))` | 三维曲面（可旋转） |
| `plot(Eq(x**2+y**2,1), (x,-2,2), (y,-2,2))` | 二维隐式曲线（等值线） |

未写区间时默认用 `[-10, 10]`。

## 综合示例

```
A = matrix([[2, 1], [1, 2]])
det(A)
eig(A)

g = sin(x)/x
limit(g, x, 0)
plot(g, (x, -15, 15))

Normal(0, 1)
```

输出会在同一个面板里依次给出：行列式、特征值/向量、极限、可交互曲线、正态分布的 PDF/CDF 交互图——
每条都带中文标题。

## 工作原理（简述）

```
数学块文本
  → 前端生成极小 bootstrap（utils/guaxiNotebookMathRuntime.js）
  → Python 会话: import guaximath_engine; guaximath_engine.run(source, options, globals())
      · AST 逐行解析，自动符号，命令库求值/绘图
      · 工作区变量持久化在会话全局 __GUAXIMATH_WS__
  → 返回 display：{type:'markdown'} 走 MathJax / {type:'html'} 走 Plotly iframe
```

引擎以真实 Python 模块随应用一起物化、`import` 一次常驻会话（性能）；
若未物化，前端会用内联源码兜底，保证可用。新增学科只需在引擎里加命令、在速查菜单加一条模板。

## 命令速查（阶段二：复变函数 / 数学物理方法）

### 复变函数
| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `residue(f, z, z0)` | `residue(1/(z**2+1), z, I)` | 留数 |
| `laurent(f, z, z0, n)` | `laurent(1/(z*(z-1)), z, 0, 6)` | 洛朗 / 幂级数展开（含留数） |
| `contour_integral(f, z, poles=[...])` | `contour_integral(1/(z**2+1), z, poles=[I])` | 围道积分（留数定理 2πi·Σ Res） |
| `conformal(f, z)` | `conformal(z**2, z)` | 共形映射：给出 f'(z)（f'≠0 处保角） |
| `cauchy_riemann(u, v, x, y)` | `cauchy_riemann(x**2-y**2, 2*x*y, x, y)` | 柯西–黎曼方程 |

### 数学物理方法
| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `laplace / inverse_laplace` | `laplace(exp(-2*t), t, s)` | 拉普拉斯（逆）变换 |
| `fourier / inverse_fourier` | `fourier(exp(-x**2), x, k)` | 傅里叶（逆）变换 |
| `mellin(f, x, s)` | `mellin(exp(-x), x, s)` | 梅林变换 |
| `besselj/legendre/hermite/laguerre/chebyshevt` | `besselj(0, x)` | 特殊函数 |
| `separate_variables(expr)` | `separate_variables(x*y + x)` | 分离变量 |
| `greens_function(x, xi, L)` | `greens_function(x, xi, 1)` | 1D Dirichlet 拉普拉斯算子的格林函数 |

## 命令速查（阶段三：张量分析 / 微分几何）

> 约定：度规 `g` 为 n×n 矩阵，坐标写成符号列表 `[r, theta]`；曲面用参数化向量 `r(u,v)`。

| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `metric(rows, coords)` | `g = metric([[1,0],[0,r**2]], [r, theta])` | 定义度规张量 |
| `christoffel(g, coords)` | `christoffel(g, [r, theta])` | Christoffel 符号 Γ^k_{ij} |
| `riemann(g, coords)` | `riemann(g, [r, theta])` | Riemann 曲率张量 R^l_{ijk} |
| `ricci / ricci_scalar` | `ricci_scalar(g, [theta, phi])` | Ricci 张量 / 标量曲率（球面 R=2/a²） |
| `geodesic_equations(g, coords)` | `geodesic_equations(g, [r, theta])` | 测地线方程组 |
| `first_/second_fundamental_form(r, (u,v))` | `first_fundamental_form([u, v, u**2+v**2], (u, v))` | 第一 / 第二基本形式 |
| `gaussian_curvature / mean_curvature(r, (u,v))` | `gaussian_curvature([a*cos(u)*sin(v), a*sin(u)*sin(v), a*cos(v)], (u, v))` | 高斯 / 平均曲率 |

## 命令速查（阶段四：矩阵论 / 泛函分析）

| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `jordan(A)` | `jordan(matrix([[2,1],[0,2]]))` | Jordan 标准形 A = P J P⁻¹ |
| `matrix_exp(A)` | `matrix_exp(matrix([[0,1],[0,0]]))` | 矩阵指数（符号；失败回退数值） |
| `schur(A)` | `schur(matrix([[2,1],[1,2]]))` | Schur 分解（数值） |
| `is_diagonalizable / cond(A)` | `cond(matrix([[2,0],[0,3]]))` | 可对角化判定 / 条件数 |
| `inner_product(u, v, ...)` | `inner_product(x, x, var=x, a=-1, b=1)` | 向量内积 / 函数 L² 内积 |
| `norm(v, ...)` | `norm(matrix([3,4]))` | 范数（向量 / 函数 L²） |
| `gram_schmidt([...])` | `gram_schmidt([matrix([1,1,0]), matrix([1,0,1])])` | 正交化（`orthonormal=True` 单位化） |
| `operator_spectrum(A)` | `operator_spectrum(matrix([[2,0],[0,-3]]))` | 算子谱（特征值 + 谱半径） |
| `l2_approx(f, basis, var, a, b, n)` | `l2_approx(exp(x), 'legendre', x, -1, 1, 3)` | L² 最佳逼近（legendre / fourier 基） |

## 命令速查（数值计算 / 仿真）

> 数值计算依赖 `scipy`；这些命令给出**数值**结果，并附「收敛过程」表与残差-迭代交互图。

| 命令 | 示例 | 说明 |
| --- | --- | --- |
| `nintegrate(f, (x,a,b))` | `nintegrate(exp(-x**2), (x, -5, 5))` | 数值积分（quad，附误差） |
| `nsolve(f, x0)` | `nsolve(x**3 - x - 2, 1.5)` | 数值求根（Newton，附收敛历史） |
| `minimize(f, x0, method=..)` | `minimize((x-3)**2 + 1, 0)` | 数值优化（附迭代历史） |
| `lstsq(A, b)` | `lstsq(matrix([[1,1],[1,2],[1,3]]), matrix([1,2,2]))` | 最小二乘解 |
| `odeint(rhs, y0, (t,a,b), n=..)` | `odeint(-y, 1, (t, 0, 5))` | ODE 数值解（标量 / 系统，画轨迹） |
| `phase_portrait([f,g], (x,..), (y,..))` | `phase_portrait([y, -x], (x,-3,3), (y,-3,3))` | 二维相图 / 向量场（仿真，轻量） |

> 仿真与几何当前为**轻量但可扩展**版本：仿真先支持 ODE 轨迹与相图，几何先支持核心曲率量；
> 引擎按学科分节（`_commands_<subject>`），新增能力 = 加一个命令函数，不影响其它学科。

## carrier 升级：内联 → 右侧面板 → 独立工作台

数学块默认把所有结果**内联**显示在单元格下方。对「重型」结果（数值解的迭代历史、ODE/相图等仿真），
引擎会额外登记一个 **carrier**（活在会话里，不进显示结果），于是可以逐级升级承载：

1. **内联**（默认）：公式、矩阵、单张图、收敛表都在单元格里。
2. **右侧面板**：选中数学块后可在右侧打开「数学工作区」面板，查看工作区变量与可复跑的 carrier；
   便宜的参数微调（精度 / 容差）在这里就地重跑。
3. **独立工作台窗口**：最重的仿真回放 / 多参数扫描在独立窗口里进行（参数滑块、收敛动画、逐帧回放）。

> 该承载层是数学模块**独立**实现的，与探索者模式里 Python Cell 的右侧面板 / 变量工作台
> 互不影响、互不复用——只是同样利用了「右侧区域」与「新窗口」这两处通用位置。

## 故障排查

| 现象 | 可能原因 / 处理 |
| --- | --- |
| 提示「语法错误」 | 检查括号是否匹配、命令拼写；多行程序确保每行独立成立 |
| 分布相关报错 | 运行时缺 `scipy`，按提示安装后重试 |
| 图不显示 / 空白 | 确认输出面板未折叠；交互图较大，首次渲染稍慢 |
| 旧 LaTeX 公式结果异常 | 复杂 LaTeX 建议改写成命令式写法，或用 `tex(r"...")` |
