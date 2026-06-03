const normalizeGuideText = (value = '') => String(value || '').trim()

const lowerGuideText = (value = '') => normalizeGuideText(value).toLowerCase()

const GUIDE_QUERY_ALIASES = {
  数组: ['array', 'ndarray', 'numpy'],
  矩阵: ['array', 'matrix', 'numpy'],
  绘图: ['plot', 'matplotlib', 'pyplot', 'figure'],
  图像: ['plot', 'matplotlib', 'figure'],
  列表: ['list'],
  字典: ['dict', 'dictionary'],
  集合: ['set'],
  元组: ['tuple'],
  字符串: ['str', 'string'],
  函数: ['function', 'def'],
  参数: ['argument', 'parameter'],
  模块: ['module', 'import'],
  导入: ['import', 'module'],
  循环: ['for', 'while', 'loop'],
  条件: ['if', 'else', 'match'],
  异常: ['exception', 'try', 'except'],
  文件: ['file', 'open', 'pathlib'],
  路径: ['path', 'pathlib', 'os.path'],
  正则: ['re', 'regex', 'regular expression'],
  时间: ['datetime', 'time'],
  随机: ['random'],
  统计: ['statistics', 'mean', 'median'],
  异步: ['asyncio', 'async', 'await'],
  array: ['数组'],
  ndarray: ['数组', 'numpy'],
  plot: ['绘图', 'matplotlib'],
  pyplot: ['绘图', 'matplotlib'],
  list: ['列表'],
  dict: ['字典'],
  tuple: ['元组'],
  set: ['集合'],
  function: ['函数', 'def'],
  import: ['导入', '模块'],
  asyncio: ['异步'],
  // —— 数学块（命令式工作区）别名：中文 ↔ 命令名，提升离线检索命中率 —— //
  极限: ['limit'],
  求导: ['diff', 'derivative'],
  导数: ['diff', 'derivative'],
  积分: ['integrate', 'nintegrate'],
  级数: ['series', 'taylor'],
  泰勒: ['taylor', 'series'],
  方程: ['solve', 'eq'],
  求解: ['solve'],
  微分方程: ['dsolve', 'derivative'],
  特征值: ['eig', 'eigenvalues'],
  特征向量: ['eig'],
  行列式: ['det'],
  分布: ['normal', 'binomial', 'distribution'],
  正态: ['normal'],
  二项: ['binomial'],
  期望: ['expectation'],
  方差: ['variance'],
  复变: ['residue', 'laurent', 'conformal'],
  留数: ['residue'],
  洛朗: ['laurent'],
  共形: ['conformal'],
  围道: ['contour_integral', 'residue'],
  变换: ['laplace', 'fourier', 'mellin', 'transform'],
  拉普拉斯: ['laplace'],
  傅里叶: ['fourier'],
  梅林: ['mellin'],
  特殊函数: ['besselj', 'legendre', 'hermite'],
  分离变量: ['separate_variables'],
  格林函数: ['greens_function'],
  张量: ['tensor', 'christoffel', 'riemann', 'ricci'],
  度规: ['metric'],
  曲率: ['ricci', 'gaussian_curvature', 'curvature'],
  标量曲率: ['ricci_scalar'],
  测地线: ['geodesic_equations'],
  微分几何: ['metric', 'christoffel', 'curvature', 'geodesic_equations'],
  矩阵指数: ['matrix_exp'],
  可对角化: ['is_diagonalizable'],
  条件数: ['cond'],
  内积: ['inner_product'],
  正交化: ['gram_schmidt'],
  范数: ['norm'],
  算子谱: ['operator_spectrum'],
  逼近: ['l2_approx'],
  投影: ['project'],
  数值: ['numerical', 'nintegrate', 'nsolve', 'minimize', 'odeint', 'lstsq'],
  求根: ['nsolve'],
  优化: ['minimize'],
  最小二乘: ['lstsq'],
  仿真: ['odeint', 'phase_portrait', 'simulation'],
  相图: ['phase_portrait'],
  向量场: ['phase_portrait'],
  轨迹: ['odeint', 'trajectory'],
  工作台: ['workbench', 'carrier'],
  面板: ['workbench', 'carrier'],
}

export const NOTEBOOK_USAGE_GUIDE_CHANNELS = [
  {
    key: 'python',
    label: 'Python',
    description: '语法、标准库、数据分析与绘图模块',
  },
  {
    key: 'math',
    label: '数学 / LaTeX',
    description: 'LaTeX 公式、SymPy 表达式与数学块工具',
  },
]

// 设计说明：
// 1. 这里先用结构化索引承载离线帮助内容，而不是把说明散落在组件模板里。
// 2. sourceUrl 保留官方文档来源，后续接入打包后的 Python / SymPy / LaTeX 文档时，可由导入器批量生成同结构条目。
// 3. 每个条目都提供 examples，组件可以直接插入当前 Cell，避免用户看懂后还要手动重写一遍。
export const NOTEBOOK_USAGE_GUIDE_ENTRIES = [
  {
    id: 'python-syntax-basics',
    channel: 'python',
    group: 'Python 语法',
    title: '变量、循环与函数',
    summary: '适合快速回忆 Python 基础语法，尤其是 Notebook Cell 里组织小段实验代码。',
    sourceTitle: 'Python 官方文档：Tutorial',
    sourceUrl: 'https://docs.python.org/3/tutorial/',
    keywords: ['python', '语法', '变量', '循环', '函数', 'def', 'for', 'if'],
    examples: [
      {
        label: '函数和循环',
        code: [
          'def square_sum(values):',
          '    total = 0',
          '    for value in values:',
          '        total += value ** 2',
          '    return total',
          '',
          'square_sum([1, 2, 3, 4])',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'python-builtin-containers',
    channel: 'python',
    group: 'Python 语法',
    title: 'list / dict / set 常用操作',
    summary: '整理列表推导式、字典访问和集合去重，适合处理轻量数据。',
    sourceTitle: 'Python 官方文档：Built-in Types',
    sourceUrl: 'https://docs.python.org/3/library/stdtypes.html',
    keywords: ['list', 'dict', 'set', '列表', '字典', '集合', '推导式'],
    examples: [
      {
        label: '列表和字典',
        code: [
          'scores = {"Alice": 92, "Bob": 81, "Cindy": 97}',
          'passed = {name: score for name, score in scores.items() if score >= 90}',
          'names = list(passed.keys())',
          'names',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'python-pandas-dataframe',
    channel: 'python',
    group: '数据分析模块',
    title: 'pandas DataFrame 查询与聚合',
    summary: '适合表格数据筛选、分组统计、排序和导出 Markdown 表格。',
    sourceTitle: 'pandas 官方文档：User Guide',
    sourceUrl: 'https://pandas.pydata.org/docs/user_guide/',
    keywords: ['pandas', 'dataframe', 'df', 'groupby', 'query', '表格', '聚合'],
    examples: [
      {
        label: '分组统计',
        code: [
          'import pandas as pd',
          '',
          'df = pd.DataFrame({',
          '    "group": ["A", "A", "B", "B"],',
          '    "value": [12, 18, 9, 21],',
          '})',
          'summary = df.groupby("group", as_index=False)["value"].mean()',
          'summary',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'python-numpy-array',
    channel: 'python',
    group: '科学计算模块',
    title: 'NumPy 数组与向量化计算',
    summary: '适合生成数值序列、做批量计算，并作为绘图或拟合的输入。',
    sourceTitle: 'NumPy 官方文档：User Guide',
    sourceUrl: 'https://numpy.org/doc/stable/user/',
    keywords: ['numpy', 'np', 'array', 'linspace', '向量化', '数组'],
    examples: [
      {
        label: '向量化计算',
        code: [
          'import numpy as np',
          '',
          'x = np.linspace(-3, 3, 200)',
          'y = np.sin(x) / np.where(x == 0, 1, x)',
          'y[:5]',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'python-matplotlib-plot',
    channel: 'python',
    group: '绘图模块',
    title: 'Matplotlib 绘制曲线',
    summary: '适合在 Python Cell 中绘制函数、实验结果和简单图表。',
    sourceTitle: 'Matplotlib 官方文档：Pyplot tutorial',
    sourceUrl: 'https://matplotlib.org/stable/tutorials/pyplot.html',
    keywords: ['matplotlib', 'plt', 'plot', '绘图', '曲线', '图像'],
    examples: [
      {
        label: '曲线图',
        code: [
          'import numpy as np',
          'import matplotlib.pyplot as plt',
          '',
          'x = np.linspace(-10, 10, 400)',
          'y = np.sin(x) / x',
          'plt.plot(x, y)',
          'plt.grid(True)',
          'plt.show()',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'math-getting-started',
    channel: 'math',
    group: '快速上手',
    order: 1,
    title: '数学工作区怎么写（赋值 / 表达式 / 命令）',
    summary: '数学块是命令式数学 REPL：每行可以是赋值（f = sin(x)/x）、裸表达式（自动以 LaTeX 显示结果），或命令（plot(...)）。未定义的名字自动当作符号，无需声明；幂写 ** 或 ^，乘号建议显式写 *（2x、3sin(x) 也能识别）。多行共享同一持久工作区，跨多次运行、跨多个数学块复用，也能读取前面 Python Cell 里的变量。注意：数学块禁用 import / 文件 / 网络，专注数学计算。',
    sourceTitle: 'SymPy 官方文档：Tutorial',
    sourceUrl: 'https://docs.sympy.org/latest/tutorials/intro-tutorial/',
    keywords: ['上手', '入门', '赋值', '表达式', '命令', '工作区', 'repl', '符号', '幂', '乘号', 'workspace', 'sympy'],
    examples: [
      {
        label: '一段完整示例',
        code: [
          'f = sin(x)/x',
          'limit(f, x, 0)',
          'diff(f, x)',
          'integrate(x**2, x)',
          'plot(f, (x, -15, 15))',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'math-run-preview',
    channel: 'math',
    group: '快速上手',
    order: 2,
    title: '运行、预览模式与命令菜单',
    summary: '工具栏 ▶ 运行整段：符号结果走 App 内置 MathJax（跟随明暗主题），图走交互式 Plotly（独立 iframe，固定亮色）。预览图标在「公式」(按 LaTeX 渲染) 与「命令」(按代码展示) 两种实时预览间切换。Σ▾ 菜单一键插入各学科命令模板；保存图标可运行并把结果写回 Markdown。右侧「面板」按钮打开数学工作区面板（见“承载层与窗口”）。',
    sourceTitle: '数学块使用说明',
    sourceUrl: '',
    keywords: ['运行', '预览', '公式', '命令', '菜单', '写回', 'plotly', 'mathjax', '主题'],
    examples: [
      {
        label: '矩阵 + 特征值',
        code: [
          'A = matrix([[2, 1], [1, 2]])',
          'det(A)',
          'eig(A)',
        ].join('\n'),
      },
    ],
  },
  {
    id: 'math-calculus',
    channel: 'math',
    group: '微积分',
    order: 10,
    title: '极限 / 求导 / 积分 / 级数 / 解方程',
    summary: 'limit(f, x, a)（可加 dir="+"/"-" 单侧）；diff(f, x[, n]) 求 n 阶导；integrate(f, x) 不定积分、integrate(f, (x, a, b)) 定积分；taylor / series(f, x, x0, n) 展开；solve(eq[, vars=...]) 解方程或方程组；summation / product(f, k, a, b) 求和连乘。变量可省略，默认取表达式里的自由符号；常量有 pi、E、I、oo。',
    sourceTitle: 'SymPy 官方文档：Calculus',
    sourceUrl: 'https://docs.sympy.org/latest/tutorials/intro-tutorial/calculus.html',
    keywords: ['limit', '极限', 'diff', '求导', '导数', 'integrate', '积分', 'taylor', 'series', '级数', '泰勒', 'solve', '方程', '求解', 'summation', '求和'],
    examples: [
      { label: '极限', code: 'limit(sin(x)/x, x, 0)' },
      { label: '高阶导数', code: 'diff(sin(x)*exp(x), x, 2)' },
      { label: '定积分', code: 'integrate(exp(-x**2), (x, -oo, oo))' },
      { label: '泰勒展开', code: 'taylor(cos(x), x, 0, 8)' },
      { label: '解方程组', code: 'solve([Eq(x + y, 3), Eq(x - y, 1)], vars=[x, y])' },
    ],
  },
  {
    id: 'math-dsolve',
    channel: 'math',
    group: '微积分',
    order: 11,
    title: '微分方程 dsolve',
    summary: '用 Derivative(y(x), x, n) 写导数、y(x) 表示未知函数（被当作函数调用的未知名字会自动成为未知函数），用 Eq(...) 组等式，再交给 dsolve 求解。',
    sourceTitle: 'SymPy 官方文档：Solving ODEs',
    sourceUrl: 'https://docs.sympy.org/latest/guides/solving/solve-ode.html',
    keywords: ['dsolve', '微分方程', 'ode', 'Derivative', '常微分', 'y(x)'],
    examples: [
      { label: '一阶 ODE', code: 'dsolve(Eq(Derivative(y(x), x), y(x)))' },
      { label: '二阶 ODE', code: 'dsolve(Eq(Derivative(y(x), x, x) + y(x), 0))' },
    ],
  },
  {
    id: 'math-linear-algebra',
    channel: 'math',
    group: '线性代数',
    order: 20,
    title: '矩阵 / 行列式 / 逆 / 特征值 / 分解',
    summary: 'matrix([[...], [...]]) 建矩阵；det / inv / transpose / rref / rank / nullspace；eig(A) 给特征值与特征向量，eigenvalues / charpoly；分解 lu / qr / svd（svd 为数值）；linsolve(A, b) 解线性方程组；还有 eye / zeros / ones / diag。',
    sourceTitle: 'SymPy 官方文档：Matrices',
    sourceUrl: 'https://docs.sympy.org/latest/modules/matrices/matrices.html',
    keywords: ['matrix', '矩阵', 'det', '行列式', 'inv', '逆', 'eig', '特征值', '特征向量', 'rref', '秩', 'svd', 'qr', 'lu', '分解', 'linsolve', '线性方程组'],
    examples: [
      { label: '矩阵 + 行列式', code: 'A = matrix([[2, 1], [1, 2]])\ndet(A)' },
      { label: '逆与特征', code: 'A = matrix([[2, 1], [1, 2]])\ninv(A)\neig(A)' },
      { label: '解线性方程组', code: 'linsolve(matrix([[1, 1], [1, -1]]), matrix([3, 1]))' },
      { label: 'SVD（数值）', code: 'svd(matrix([[1, 2], [3, 4]]))' },
    ],
  },
  {
    id: 'math-probability',
    channel: 'math',
    group: '概率统计',
    order: 30,
    title: '常见分布与期望 / 方差',
    summary: '分布用首字母大写的构造器：Normal(μ, σ)、Uniform、Exponential、Gamma、Beta、Binomial(n, p)、Poisson(λ)、Bernoulli、Geometric——会画 PDF/PMF + CDF 交互图并给期望方差；expectation(...) / variance(...) 取数值。依赖 scipy。',
    sourceTitle: 'SciPy 官方文档：stats',
    sourceUrl: 'https://docs.scipy.org/doc/scipy/reference/stats.html',
    keywords: ['分布', '概率', '统计', 'Normal', '正态', 'Binomial', '二项', 'Poisson', '泊松', 'expectation', 'variance', '期望', '方差', 'scipy'],
    examples: [
      { label: '正态分布', code: 'Normal(0, 1)' },
      { label: '二项分布', code: 'Binomial(10, 0.4)' },
      { label: '期望 / 方差', code: 'expectation(Normal(0, 1))' },
    ],
  },
  {
    id: 'math-plot',
    channel: 'math',
    group: '绘图',
    order: 40,
    title: '交互式绘图 plot',
    summary: 'plot(f, (x, a, b)) 一维曲线（复值自动分实部 / 虚部）；plot([f, g], (x, a, b)) 多条对比；plot(f, (x, ..), (y, ..)) 三维曲面（可旋转）；plot(Eq(...), (x, ..), (y, ..)) 二维隐式曲线。不写区间默认 [-10, 10]。图在独立 iframe 用亮色主题渲染以保证清晰。',
    sourceTitle: 'Plotly 官方文档',
    sourceUrl: 'https://plotly.com/python/',
    keywords: ['plot', '绘图', '曲线', '图像', '三维', '曲面', '隐式', 'plotly'],
    examples: [
      { label: '一维曲线', code: 'plot(sin(x)/x, (x, -15, 15))' },
      { label: '多条曲线', code: 'plot([sin(x), cos(x)], (x, -pi, pi))' },
      { label: '三维曲面', code: 'plot(x**2 + y**2, (x, -3, 3), (y, -3, 3))' },
      { label: '隐式曲线', code: 'plot(Eq(x**2 + y**2, 1), (x, -2, 2), (y, -2, 2))' },
    ],
  },
  {
    id: 'math-complex',
    channel: 'math',
    group: '复变函数',
    order: 50,
    title: '留数 / 洛朗级数 / 共形映射',
    summary: 'residue(f, z, z0) 留数；laurent(f, z, z0, n) 洛朗展开（附留数）；contour_integral(f, z, poles=[...]) 用留数定理算围道积分（2πi·Σ Res）；conformal(f, z) 给 f′(z)（f′≠0 处保角）；cauchy_riemann(u, v, x, y) 给柯西–黎曼方程。默认复变量是 z。',
    sourceTitle: 'SymPy 官方文档：Series / residue',
    sourceUrl: 'https://docs.sympy.org/latest/modules/series/series.html',
    keywords: ['复变', 'residue', '留数', 'laurent', '洛朗', 'conformal', '共形', 'cauchy', 'riemann', '柯西黎曼', '围道', 'contour_integral', 'z'],
    examples: [
      { label: '留数', code: 'residue(1/(z**2 + 1), z, I)' },
      { label: '洛朗展开', code: 'laurent(1/(z*(z - 1)), z, 0, 6)' },
      { label: '围道积分', code: 'contour_integral(1/(z**2 + 1), z, poles=[I])' },
      { label: '柯西-黎曼', code: 'cauchy_riemann(x**2 - y**2, 2*x*y, x, y)' },
    ],
  },
  {
    id: 'math-physics',
    channel: 'math',
    group: '数学物理',
    order: 51,
    title: '积分变换 / 特殊函数 / 分离变量 / 格林函数',
    summary: 'laplace / inverse_laplace（t↔s）、fourier / inverse_fourier（x↔k）、mellin 积分变换；特殊函数 besselj / bessely / legendre / hermite / laguerre / chebyshevt 等可直接当函数调用；separate_variables(expr) 分离变量；greens_function(x, xi, L) 给一维 Dirichlet 拉普拉斯算子 -d²/dx² 在 [0, L] 上的格林函数。',
    sourceTitle: 'SymPy 官方文档：Integral transforms',
    sourceUrl: 'https://docs.sympy.org/latest/modules/integrals/integrals.html',
    keywords: ['数学物理', 'laplace', '拉普拉斯', 'fourier', '傅里叶', 'mellin', '梅林', '变换', 'bessel', 'legendre', '特殊函数', '分离变量', 'greens_function', '格林函数'],
    examples: [
      { label: '拉普拉斯变换', code: 'laplace(exp(-2*t), t, s)' },
      { label: '傅里叶变换', code: 'fourier(exp(-x**2), x, k)' },
      { label: '贝塞尔函数', code: 'besselj(0, x)' },
      { label: '格林函数', code: 'greens_function(x, xi, 1)' },
    ],
  },
  {
    id: 'math-geometry',
    channel: 'math',
    group: '张量与微分几何',
    order: 60,
    title: '度规 / Christoffel / 曲率 / 测地线',
    summary: '先 g = metric([[...]], [坐标]) 定义度规（坐标写成符号列表如 [r, theta]）；christoffel(g, coords) 给 Γ^k_ij；riemann / ricci / ricci_scalar 求曲率（球面标量曲率 = 2/a²）；geodesic_equations(g, coords) 给测地线方程。曲面用参数化向量 r(u, v)：first_/second_fundamental_form、gaussian_curvature、mean_curvature。',
    sourceTitle: 'SymPy 官方文档：Matrices / diffgeom',
    sourceUrl: 'https://docs.sympy.org/latest/modules/diffgeom.html',
    keywords: ['张量', '微分几何', '度规', 'metric', 'christoffel', '黎曼', 'riemann', 'ricci', '曲率', '标量曲率', '测地线', 'geodesic', '基本形式', '高斯曲率', '平均曲率'],
    examples: [
      { label: '极坐标 Christoffel', code: 'g = metric([[1, 0], [0, r**2]], [r, theta])\nchristoffel(g, [r, theta])' },
      { label: '球面标量曲率', code: 'g = metric([[a**2, 0], [0, a**2*sin(theta)**2]], [theta, phi])\nricci_scalar(g, [theta, phi])' },
      { label: '测地线方程', code: 'g = metric([[1, 0], [0, r**2]], [r, theta])\ngeodesic_equations(g, [r, theta])' },
      { label: '曲面高斯曲率', code: 'gaussian_curvature([a*cos(u)*sin(v), a*sin(u)*sin(v), a*cos(v)], (u, v))' },
    ],
  },
  {
    id: 'math-matrix-theory',
    channel: 'math',
    group: '矩阵论',
    order: 70,
    title: 'Jordan 标准形 / 矩阵指数 / Schur',
    summary: 'jordan(A) 给 Jordan 标准形（P, J）；matrix_exp(A) 矩阵指数（符号失败时回退数值）；schur(A) Schur 分解（数值）；is_diagonalizable(A) 判定可对角化；cond(A) 条件数。',
    sourceTitle: 'SymPy 官方文档：Matrices',
    sourceUrl: 'https://docs.sympy.org/latest/modules/matrices/matrices.html',
    keywords: ['矩阵论', 'jordan', '标准形', 'matrix_exp', '矩阵指数', 'schur', '分解', '可对角化', 'is_diagonalizable', '条件数', 'cond'],
    examples: [
      { label: 'Jordan 标准形', code: 'jordan(matrix([[2, 1], [0, 2]]))' },
      { label: '矩阵指数', code: 'matrix_exp(matrix([[0, 1], [0, 0]]))' },
      { label: 'Schur 分解', code: 'schur(matrix([[2, 1], [1, 2]]))' },
    ],
  },
  {
    id: 'math-functional',
    channel: 'math',
    group: '泛函分析',
    order: 71,
    title: '内积 / 正交化 / 算子谱 / L² 逼近',
    summary: 'inner_product(u, v[, var, a, b, weight]) 向量内积或函数 L² 内积；norm(v) 范数；gram_schmidt([v1, v2, ...]) 正交化（orthonormal=True 单位化）；operator_spectrum(A) 给特征值 + 谱半径；l2_approx(f, "legendre"|"fourier", x, a, b, n) L² 最佳逼近；project(u, v) 投影。',
    sourceTitle: 'SymPy 官方文档：Matrices',
    sourceUrl: 'https://docs.sympy.org/latest/modules/matrices/matrices.html',
    keywords: ['泛函分析', '内积', 'inner_product', '正交化', 'gram_schmidt', '范数', 'norm', '算子谱', 'operator_spectrum', '谱半径', 'l2_approx', '逼近', '投影', 'project'],
    examples: [
      { label: '函数 L² 内积', code: 'inner_product(x, x, var=x, a=-1, b=1)' },
      { label: 'Gram-Schmidt 正交化', code: 'gram_schmidt([matrix([1, 1, 0]), matrix([1, 0, 1])])' },
      { label: 'L² 最佳逼近', code: "l2_approx(exp(x), 'legendre', x, -1, 1, 3)" },
    ],
  },
  {
    id: 'math-numerical',
    channel: 'math',
    group: '数值计算',
    order: 80,
    title: '数值积分 / 求根 / 优化 / ODE',
    summary: '给出数值结果，并附「收敛过程」表与残差-迭代交互图。nintegrate(f, (x, a, b)) 数值积分；nsolve(f, x0[, tol, maxiter]) 牛顿求根；minimize(f, x0[, method, tol, maxiter]) 优化；lstsq(A, b) 最小二乘；odeint(rhs, y0, (t, a, b)[, n]) ODE 数值解并画轨迹。依赖 scipy。重型结果会登记到右侧「数学工作区」面板，可在独立工作台里调参复跑。',
    sourceTitle: 'SciPy 官方文档：optimize / integrate',
    sourceUrl: 'https://docs.scipy.org/doc/scipy/reference/optimize.html',
    keywords: ['数值', '计算', 'nintegrate', '数值积分', 'nsolve', '求根', '牛顿', 'minimize', '优化', 'lstsq', '最小二乘', 'odeint', 'ode', '收敛', 'scipy'],
    examples: [
      { label: '数值积分', code: 'nintegrate(exp(-x**2), (x, -5, 5))' },
      { label: '数值求根', code: 'nsolve(x**3 - x - 2, 1.5)' },
      { label: '数值优化', code: 'minimize((x - 3)**2 + 1, 0)' },
      { label: 'ODE 数值解', code: 'odeint(-y, 1, (t, 0, 5))' },
    ],
  },
  {
    id: 'math-simulation',
    channel: 'math',
    group: '仿真',
    order: 81,
    title: '动力系统：ODE 系统 / 相图',
    summary: 'odeint([f, g], [x0, y0], (t, a, b), vars=[x, y]) 解二维系统并画分量轨迹；phase_portrait([f, g], (x, ..), (y, ..)) 画相图 / 向量场。解数组会暂存到会话，可在数学工作台里逐帧回放（当前为轻量骨架，后续可扩展为动画）。',
    sourceTitle: 'SciPy 官方文档：integrate.solve_ivp',
    sourceUrl: 'https://docs.scipy.org/doc/scipy/reference/generated/scipy.integrate.solve_ivp.html',
    keywords: ['仿真', '动力系统', '相图', 'phase_portrait', '向量场', '轨迹', 'trajectory', 'odeint', '系统', '回放'],
    examples: [
      { label: 'ODE 系统（谐振子）', code: 'odeint([y, -x], [1, 0], (t, 0, 6.283), vars=[x, y])' },
      { label: '相图 / 向量场', code: 'phase_portrait([y, -x], (x, -3, 3), (y, -3, 3))' },
    ],
  },
  {
    id: 'math-workbench',
    channel: 'math',
    group: '承载层与窗口',
    order: 90,
    title: '数学工作区面板与数学工作台窗口',
    summary: '数学块工具栏的「面板」按钮在右侧打开「数学工作区」面板：列出会话里的工作区变量，以及可复跑的数值 / 仿真工作台（carrier）。点 carrier 的「打开数学工作台」会弹出独立窗口，提供参数滑块、按参数重新运行、收敛过程展示与（仿真）逐帧回放。该承载层与 Python Cell 的右侧面板 / 变量工作台完全独立、互不影响。先运行一个 nsolve / odeint 即可在面板里看到对应工作台。',
    sourceTitle: '数学块承载层说明',
    sourceUrl: '',
    keywords: ['面板', '工作区', '工作台', '窗口', 'carrier', '滑块', '参数', '复跑', '承载', '回放'],
    examples: [
      { label: '生成一个可调参工作台', code: 'nsolve(x**3 - x - 2, 1.5)' },
    ],
  },
  {
    id: 'math-latex',
    channel: 'math',
    group: 'LaTeX 语法',
    order: 95,
    title: 'LaTeX 写法与 tex()',
    summary: '公式预览支持常见 LaTeX；运行时旧版纯 LaTeX（如 \\frac、\\sqrt、\\partial）会尽量翻译给 SymPy。要把一段 LaTeX 当表达式参与计算，用 tex(r"\\frac{a}{b}")。复杂公式建议直接改写成命令式写法更稳。',
    sourceTitle: 'LaTeX Project：数学排版',
    sourceUrl: 'https://www.latex-project.org/help/documentation/',
    keywords: ['latex', 'frac', 'sqrt', '分式', '根号', '上标', '下标', 'tex', '公式'],
    examples: [
      { label: '分式与根号', code: '\\frac{\\sin x}{\\sqrt{x^2 + 1}}' },
      { label: '用 tex() 解析 LaTeX', code: 'tex(r"\\frac{a}{b}")' },
    ],
  },
  {
    id: 'math-tips',
    channel: 'math',
    group: '提示与排错',
    order: 99,
    title: '常见坑与提示',
    summary: '① 数学块禁用 import / 文件 / 网络，只做数学计算，需要任意脚本请用 Python Cell。② 变量在会话里持久，跨运行 / 跨块复用，也能读 Python Cell 的变量。③ 幂用 ** 或 ^，乘号建议显式 *（2x、3sin(x) 也能识别）。④ 分布 / 数值 / Schur 等依赖 scipy，缺包会给中文提示。⑤ 某行写错只在该行提示，不影响其它行。⑥ 常量 pi、E、I、oo（无穷）；未定义的名字自动当符号，被调用的未知名字当未知函数。',
    sourceTitle: '数学块使用说明',
    sourceUrl: '',
    keywords: ['提示', '排错', '坑', 'import', '禁用', '持久', '变量', 'scipy', '依赖', '常量', 'oo', 'pi', '自动符号', '错误'],
    examples: [
      { label: '自动符号 + 常量', code: 'integrate(exp(-a*x), (x, 0, oo))' },
    ],
  },
]

let pythonOfficialDocsManifestPromise = null
let pythonOfficialDocsEntriesPromise = null

const fetchGuideJson = async (url) => {
  const response = await fetch(url, { cache: 'no-cache' })
  if (!response.ok) {
    throw new Error(`读取离线文档索引失败：${response.status} ${response.statusText}`)
  }
  return response.json()
}

export const loadPythonOfficialDocsManifest = async () => {
  if (!pythonOfficialDocsManifestPromise) {
    pythonOfficialDocsManifestPromise = fetchGuideJson('/guaxi-docs/python/index.json')
  }
  return pythonOfficialDocsManifestPromise
}

export const loadPythonOfficialDocsEntries = async () => {
  if (!pythonOfficialDocsEntriesPromise) {
    pythonOfficialDocsEntriesPromise = loadPythonOfficialDocsManifest()
      .then(async (manifest) => {
        const chunks = Array.isArray(manifest?.chunks) ? manifest.chunks : []
        const chunkEntries = await Promise.all(chunks.map((chunk) => {
          const file = String(chunk?.file || '').trim()
          if (!file) return []
          return fetchGuideJson(`/guaxi-docs/python/${file}`)
        }))
        return {
          manifest,
          entries: chunkEntries.flat().filter(Boolean),
        }
      })
  }
  return pythonOfficialDocsEntriesPromise
}

const tokenizeGuideQuery = (query = '') => {
  const tokens = lowerGuideText(query)
    .split(/[\s,.;:，。；：()（）/\\[\]{}]+/)
    .map((item) => item.trim())
    .filter(Boolean)
  const expanded = []
  for (const token of tokens) {
    expanded.push(token)
    const aliases = GUIDE_QUERY_ALIASES[token] || []
    expanded.push(...aliases.map((item) => lowerGuideText(item)).filter(Boolean))
  }
  return Array.from(new Set(expanded))
}

const scoreGuideEntry = (entry, tokens = []) => {
  if (!tokens.length) return 0
  const title = lowerGuideText(entry.title)
  const summary = lowerGuideText(entry.summary)
  const group = lowerGuideText(entry.group)
  const sourceTitle = lowerGuideText(entry.sourceTitle)
  const keywords = lowerGuideText((entry.keywords || []).join(' '))
  const examples = lowerGuideText((entry.examples || []).map((item) => item.code).join(' '))
  return tokens.reduce((score, token) => {
    let nextScore = score
    if (title === token) nextScore += 24
    else if (title.includes(token)) nextScore += 12
    if (group.includes(token)) nextScore += 8
    if (keywords.includes(token)) nextScore += 8
    if (summary.includes(token)) nextScore += 4
    if (sourceTitle.includes(token)) nextScore += 3
    if (examples.includes(token)) nextScore += 2
    return nextScore
  }, 0)
}

const scorePythonOfficialDocEntry = (entry, tokens = [], rawQuery = '') => {
  const title = lowerGuideText(entry?.title)
  const kind = lowerGuideText(entry?.kind)
  const path = lowerGuideText(entry?.path)
  const pathSegments = path.split(/[/.#_-]+/).filter(Boolean)
  const headings = lowerGuideText((Array.isArray(entry?.headings) ? entry.headings : []).join(' '))
  const excerpt = lowerGuideText(entry?.excerpt)
  const searchText = lowerGuideText(entry?.searchText || [
    entry?.title,
    entry?.kind,
    entry?.path,
    ...(Array.isArray(entry?.headings) ? entry.headings : []),
    entry?.excerpt,
  ].join(' '))
  const phrase = lowerGuideText(rawQuery)
  const hasToken = (text, token) => {
    if (!token) return false
    if (/^[a-z0-9_.-]+$/i.test(token)) {
      const escaped = token.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      return new RegExp(`(^|[^a-z0-9_])${escaped}([^a-z0-9_]|$)`, 'i').test(text)
    }
    return text.includes(token)
  }

  if (!tokens.length) {
    if (path === 'tutorial/index.html') return 42
    if (path === 'library/functions.html' || path === 'library/stdtypes.html') return 38
    if (path === 'library/index.html') return 34
    if (path.startsWith('tutorial/')) return 26
    if (path.startsWith('library/')) return 22
    if (path.startsWith('reference/')) return 18
    return 0
  }

  let score = 0

  if (phrase) {
    if (title === phrase) score += 50
    else if (title.includes(phrase)) score += 28
    if (path.includes(phrase)) score += 18
    if (headings.includes(phrase)) score += 14
  }

  for (const token of tokens) {
    if (!token) continue
    if (title === token) score += 34
    else if (title.startsWith(token)) score += 24
    else if (hasToken(title, token)) score += 16

    if (pathSegments.includes(token)) score += 26
    else if (hasToken(path, token)) score += 12

    if (hasToken(headings, token)) score += 10
    if (hasToken(kind, token)) score += 5
    if (hasToken(excerpt, token)) score += 4
    if (hasToken(searchText, token)) score += 1
  }

  // 设计说明：
  // 1. Python 官方文档首页、about、404 等页面包含大量公共导航词，低权重命中时容易污染结果。
  // 2. 对这些泛页面做轻微降权，保留精确命中能力，同时让库/教程/参考章节优先浮上来。
  if (/^(404|about|bugs|copyright|download|contents|genindex|search)\.html$/.test(path)) {
    score -= 12
  }
  if (path.startsWith('library/') && (tokens.some((token) => pathSegments.includes(token)) || title.includes(tokens[0] || ''))) {
    score += 8
  }
  if (path.startsWith('tutorial/') || path.startsWith('reference/')) {
    score += 3
  }
  return Math.max(0, score)
}

export const queryNotebookUsageGuide = ({ channel = 'python', query = '', limit = 12 } = {}) => {
  const safeChannel = ['python', 'math'].includes(String(channel || '')) ? String(channel) : 'python'
  const tokens = tokenizeGuideQuery(query)
  const entries = NOTEBOOK_USAGE_GUIDE_ENTRIES
    .filter((entry) => entry.channel === safeChannel)
    .map((entry) => ({
      ...entry,
      score: scoreGuideEntry(entry, tokens),
    }))
    .filter((entry) => !tokens.length || entry.score > 0)
    // 命中分优先；同分时按 order（学科逻辑顺序）排，保证“无查询”时列表自上而下可读。
    .sort((a, b) => (
      b.score - a.score
      || ((a.order ?? 999) - (b.order ?? 999))
      || String(a.group).localeCompare(String(b.group), 'zh-Hans-CN')
    ))
  return entries.slice(0, Math.max(1, Number(limit) || 12))
}

export const queryPythonOfficialDocs = async ({ query = '', limit = 20 } = {}) => {
  const { manifest, entries } = await loadPythonOfficialDocsEntries()
  const tokens = tokenizeGuideQuery(query)
  const officialEntries = (Array.isArray(entries) ? entries : [])
    .map((entry) => {
      const score = scorePythonOfficialDocEntry(entry, tokens, query)
      return {
        ...entry,
        score,
      }
    })
    .filter((entry) => entry.score > 0)
  const curatedEntries = queryNotebookUsageGuide({ channel: 'python', query, limit: 8 })
    .map((entry) => ({
      ...entry,
      kind: entry.group || 'Python 模块帮助',
      excerpt: entry.summary,
      // 设计说明：Python 官方完整文档不包含 NumPy / pandas / matplotlib 等第三方库。
      // 这些本地结构化条目作为补充结果参与同一排序，避免模块查询落到无关的语言参考页。
      score: Number(entry.score || 0) * 10,
    }))
    .filter((entry) => !tokens.length || entry.score > 0)
  const scoredEntries = [...officialEntries, ...curatedEntries]
    .sort((a, b) => b.score - a.score || String(a.path || '').localeCompare(String(b.path || ''), 'en-US'))
    .slice(0, Math.max(1, Number(limit) || 20))
  return {
    manifest,
    entries: scoredEntries,
  }
}

export const inferNotebookUsageGuideQuery = ({ channel = 'python', content = '' } = {}) => {
  const text = lowerGuideText(content)
  if (!text) return ''
  if (channel === 'math') {
    if (text.includes('residue') || text.includes('laurent') || text.includes('conformal') || text.includes('contour')) return '复变 留数 residue laurent'
    if (text.includes('christoffel') || text.includes('metric(') || text.includes('ricci') || text.includes('geodesic') || text.includes('curvature')) return '张量 微分几何 度规 曲率'
    if (text.includes('laplace') || text.includes('fourier') || text.includes('mellin') || text.includes('bessel') || text.includes('greens')) return '数学物理 变换 特殊函数'
    if (text.includes('jordan') || text.includes('matrix_exp') || text.includes('schur')) return '矩阵论 jordan schur'
    if (text.includes('inner_product') || text.includes('gram_schmidt') || text.includes('l2_approx') || text.includes('operator_spectrum')) return '泛函分析 内积 正交化'
    if (text.includes('phase_portrait') || text.includes('trajectory')) return '仿真 相图 向量场'
    if (text.includes('nintegrate') || text.includes('nsolve') || text.includes('minimize') || text.includes('odeint') || text.includes('lstsq')) return '数值计算 nsolve odeint'
    if (text.includes('normal(') || text.includes('binomial(') || text.includes('poisson(') || text.includes('expectation')) return '概率 分布 Normal Binomial'
    if (text.includes('matrix(') || text.includes('det(') || text.includes('eig(') || text.includes('svd')) return '线性代数 矩阵 特征值'
    if (text.includes('derivative') || text.includes('dsolve') || text.includes('y(')) return '微分方程 dsolve'
    if (text.includes('limit(') || text.includes('integrate') || text.includes('taylor') || text.includes('series')) return '微积分 极限 积分 级数'
    if (text.includes('plot(')) return '绘图 plot 曲线'
    if (text.includes('eq(') || text.includes('solve')) return '方程 求解 solve'
    if (text.includes('\\frac') || text.includes('\\sqrt') || text.includes('tex(')) return 'latex frac sqrt tex'
    if (text.includes('sin') || text.includes('exp')) return '函数 表达式 绘图'
    return text.slice(0, 80)
  }
  if (text.includes('pandas') || text.includes('dataframe') || text.includes('groupby')) return 'pandas dataframe groupby'
  if (text.includes('numpy') || text.includes('np.')) return 'numpy array linspace'
  if (text.includes('matplotlib') || text.includes('plt.')) return 'matplotlib plot'
  if (text.includes('def ') || text.includes('for ')) return 'python 函数 循环'
  return text.slice(0, 80)
}

export const buildNotebookUsageAssistantPrompt = ({
  channel = 'python',
  content = '',
  guideEntry = null,
} = {}) => {
  const safeChannel = channel === 'math' ? '数学 / LaTeX' : 'Python'
  const entry = guideEntry && typeof guideEntry === 'object' ? guideEntry : null
  const lines = [
    `请作为瓜西探索者模式的${safeChannel}使用助手，基于当前 Cell 内容给出简洁、可执行的说明。`,
    '要求：优先解释接口/参数/语法，给出最小可运行示例，并指出常见坑。',
  ]
  if (entry) {
    lines.push(`参考离线条目：${entry.title}`)
    lines.push(`条目摘要：${entry.summary}`)
    if (entry.sourceTitle) {
      lines.push(`官方来源：${entry.sourceTitle} ${entry.sourceUrl || ''}`.trim())
    }
  }
  const currentContent = normalizeGuideText(content)
  if (currentContent) {
    lines.push('')
    lines.push('当前 Cell 内容：')
    lines.push('```')
    lines.push(currentContent)
    lines.push('```')
  }
  return lines.join('\n')
}
