const normalizeGuideText = (value = '') => String(value || '').trim()

const lowerGuideText = (value = '') => normalizeGuideText(value).toLowerCase()

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
    id: 'math-latex-fraction-root',
    channel: 'math',
    group: 'LaTeX 语法',
    title: '分式、根号和上下标',
    summary: '数学块支持常见 LaTeX 表达式，会在运行时尽量转换给 SymPy 解析。',
    sourceTitle: 'LaTeX Project：数学排版',
    sourceUrl: 'https://www.latex-project.org/help/documentation/',
    keywords: ['latex', 'frac', 'sqrt', '上标', '下标', '分式', '根号'],
    examples: [
      {
        label: '分式和根号',
        code: '\\frac{\\sin x}{\\sqrt{x^2 + 1}}',
      },
    ],
  },
  {
    id: 'math-sympy-expression',
    channel: 'math',
    group: 'SymPy 表达式',
    title: '函数表达式与变量',
    summary: '没有反斜杠的输入会优先按 SymPy 风格解析，更适合函数绘图和符号计算。',
    sourceTitle: 'SymPy 官方文档：Tutorial',
    sourceUrl: 'https://docs.sympy.org/latest/tutorials/intro-tutorial/',
    keywords: ['sympy', 'sin', 'cos', 'exp', '变量', '表达式', '函数'],
    examples: [
      {
        label: '函数表达式',
        code: 'sin(x) / x',
      },
    ],
  },
  {
    id: 'math-solve-equation',
    channel: 'math',
    group: '数学块工具',
    title: '求解方程',
    summary: '使用 Eq(...) 表示等式，再从数学工具菜单选择“求解方程”。',
    sourceTitle: 'SymPy 官方文档：Solvers',
    sourceUrl: 'https://docs.sympy.org/latest/guides/solving/',
    keywords: ['solve', 'equation', 'eq', '方程', '求解', '根'],
    examples: [
      {
        label: '一元方程',
        code: 'Eq(x**2 - 4, 0)',
      },
      {
        label: '方程组',
        code: 'Eq(x + y, 3), Eq(x - y, 1)',
      },
    ],
  },
  {
    id: 'math-calculus',
    channel: 'math',
    group: '数学块工具',
    title: '求导、积分与级数',
    summary: '同一个表达式可以分别用“求导”“积分”“级数展开”处理，变量默认是 x。',
    sourceTitle: 'SymPy 官方文档：Calculus',
    sourceUrl: 'https://docs.sympy.org/latest/tutorials/intro-tutorial/calculus.html',
    keywords: ['derivative', 'integral', 'series', '求导', '积分', '级数'],
    examples: [
      {
        label: '微积分表达式',
        code: 'exp(-x**2) * sin(x)',
      },
    ],
  },
  {
    id: 'math-distribution',
    channel: 'math',
    group: '数学块工具',
    title: '统计分布',
    summary: '分布工具使用 scipy.stats 风格输入，例如 norm(0, 1) 或 binom(10, 0.5)。',
    sourceTitle: 'SciPy 官方文档：stats',
    sourceUrl: 'https://docs.scipy.org/doc/scipy/reference/stats.html',
    keywords: ['distribution', 'stats', 'norm', 'binom', '分布', '概率', '统计'],
    examples: [
      {
        label: '正态分布',
        code: 'norm(0, 1)',
      },
      {
        label: '二项分布',
        code: 'binom(10, 0.5)',
      },
    ],
  },
  {
    id: 'math-dsolve-transform',
    channel: 'math',
    group: '数学块工具',
    title: '微分方程与积分变换',
    summary: '微分方程使用 Derivative(...) / y(x)，拉普拉斯和傅里叶变换适合 t 变量表达式。',
    sourceTitle: 'SymPy 官方文档：ODE / Integral transforms',
    sourceUrl: 'https://docs.sympy.org/latest/modules/integrals/integrals.html',
    keywords: ['dsolve', 'laplace', 'fourier', 'Derivative', '微分方程', '变换'],
    examples: [
      {
        label: '常微分方程',
        code: 'Eq(Derivative(y(x), x), y(x))',
      },
      {
        label: '拉普拉斯变换',
        code: 'exp(-t) * sin(t)',
      },
    ],
  },
]

let pythonOfficialDocsManifestPromise = null
let pythonOfficialDocsEntriesPromise = null

const fetchGuideJson = async (url) => {
  const response = await fetch(url, { cache: 'force-cache' })
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
  return lowerGuideText(query)
    .split(/[\s,.;:，。；：()（）/\\[\]{}]+/)
    .map((item) => item.trim())
    .filter(Boolean)
}

const scoreGuideEntry = (entry, tokens = []) => {
  if (!tokens.length) return 0
  const haystack = lowerGuideText([
    entry.title,
    entry.summary,
    entry.group,
    entry.sourceTitle,
    ...(entry.keywords || []),
    ...(entry.examples || []).map((item) => item.code),
  ].join(' '))
  return tokens.reduce((score, token) => score + (haystack.includes(token) ? 1 : 0), 0)
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
    .sort((a, b) => b.score - a.score || String(a.group).localeCompare(String(b.group), 'zh-Hans-CN'))
  return entries.slice(0, Math.max(1, Number(limit) || 12))
}

export const queryPythonOfficialDocs = async ({ query = '', limit = 20 } = {}) => {
  const { manifest, entries } = await loadPythonOfficialDocsEntries()
  const tokens = tokenizeGuideQuery(query)
  const scoredEntries = (Array.isArray(entries) ? entries : [])
    .map((entry) => {
      const searchText = String(entry?.searchText || [
        entry?.title,
        entry?.kind,
        entry?.path,
        ...(Array.isArray(entry?.headings) ? entry.headings : []),
        entry?.excerpt,
      ].join(' ')).toLowerCase()
      const score = tokens.length
        ? tokens.reduce((total, token) => total + (searchText.includes(token) ? 1 : 0), 0)
        : 1
      return {
        ...entry,
        score,
      }
    })
    .filter((entry) => entry.score > 0)
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
    if (text.includes('derivative') || text.includes('dsolve') || text.includes('y(')) return '微分方程 derivative dsolve'
    if (text.includes('eq(') || text.includes('=')) return '方程 求解 Eq solve'
    if (text.includes('norm(') || text.includes('binom(')) return '统计 分布 norm binom'
    if (text.includes('\\frac') || text.includes('\\sqrt')) return 'latex frac sqrt'
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
