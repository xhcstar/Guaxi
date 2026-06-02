# -*- coding: utf-8 -*-
# ============================================================================
# guaximath_engine.py —— 探索笔记本「数学块」高级数学工作区引擎
# ----------------------------------------------------------------------------
# 设计目标（取代旧的 JS 字符串拼接式代码生成器 guaxiNotebookMathRuntime.js）：
#   1. 数学块是一个「命令式数学工作区（REPL）」：每行可以是
#        - 赋值       f = sin(x)/x
#        - 裸表达式   limit(f, x, 0)        # 自动以 LaTeX 渲染结果
#        - 命令       plot(f, (x,-10,10))   # 交互式 Plotly 图
#      多行共享一个「持久符号工作区」，可组合多步运算。
#   2. 引擎是一份真实、可测试的 Python 模块（不是 JS 字符串），常驻持久会话，
#      被 `import guaximath_engine` 后一次加载；后续每次运行只跑 `run()`。
#   3. 所有符号计算委托 SymPy，数值/绘图委托 NumPy + Plotly（交互）/ Matplotlib（兜底）。
#   4. 面向用户文案均为中文。
#
# 输出协议：`run()` 返回一个 display 字典，bootstrap 会把它赋给会话全局
#   `__guaxi_plotly_display__`，worker 统一转发为 result_json.__guaxi_display__：
#     - { type:'markdown', mime:'text/markdown', markdown }  纯符号结果（走 App MathJax）。
#     - { type:'html',     mime:'text/html',     html }      含交互图（Plotly + 行内公式 SVG）。
#     - { type:'image',    mime:'image/png',     data_base64 } Plotly 不可用时的静态兜底。
# ============================================================================

from __future__ import annotations

import ast
import base64
import io
import json
import re
import sys

ENGINE_VERSION = "1.0.0"

# ---------------------------------------------------------------------------
# 基础库（引擎只加载一次，这里直接 import；缺失时给出中文提示而非崩溃）
# ---------------------------------------------------------------------------
import sympy as sp  # noqa: E402
import numpy as np  # noqa: E402
from sympy.parsing.sympy_parser import (  # noqa: E402
    parse_expr,
    standard_transformations,
    implicit_multiplication_application,
    convert_xor,
)

_TRANSFORMS = standard_transformations + (
    implicit_multiplication_application,
    convert_xor,
)

try:
    import plotly.graph_objects as go  # noqa: E402
    import plotly.io as pio  # noqa: E402
    _HAS_PLOTLY = True
except Exception:  # pragma: no cover - 运行环境缺包时降级
    go = None
    pio = None
    _HAS_PLOTLY = False


# ---------------------------------------------------------------------------
# 依赖自愈：parse_latex 需要 antlr4-python3-runtime（sympy 1.14 要求 4.11.x）。
# 仅当探测失败时按需补装（清华镜像）；失败也不报错，自动走纯文本兜底解析。
# 带模块级缓存标志，避免每次 run() 重复探测。
# ---------------------------------------------------------------------------
_LATEX_BACKEND_READY = None


def _probe_latex_backend() -> bool:
    try:
        from sympy.parsing.latex import parse_latex
        parse_latex("x^{2}")
        return True
    except Exception:
        return False


def _ensure_latex_backend() -> bool:
    global _LATEX_BACKEND_READY
    if _LATEX_BACKEND_READY is not None:
        return _LATEX_BACKEND_READY
    if _probe_latex_backend():
        _LATEX_BACKEND_READY = True
        return True
    try:
        import subprocess
        subprocess.run(
            [sys.executable, "-m", "pip", "install",
             "antlr4-python3-runtime==4.11.1",
             "-i", "https://pypi.tuna.tsinghua.edu.cn/simple",
             "--trusted-host", "pypi.tuna.tsinghua.edu.cn",
             "--quiet", "--disable-pip-version-check"],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=180,
        )
        import importlib
        importlib.invalidate_caches()
        for _m in [m for m in list(sys.modules) if m == "antlr4" or m.startswith("antlr4.")]:
            sys.modules.pop(_m, None)
        _LATEX_BACKEND_READY = _probe_latex_backend()
    except Exception:
        _LATEX_BACKEND_READY = False
    return _LATEX_BACKEND_READY


# ===========================================================================
# 异常 / 可渲染对象协议
# ===========================================================================
class GuaxiMathError(Exception):
    """面向用户的、可读的数学错误（中文）。"""


class Renderable:
    """命令可返回该协议对象，自行决定如何渲染成结果块。

    子类实现 render(ctx)：往 ctx.blocks 追加块（latex / text / figure）。
    """

    def render(self, ctx):  # pragma: no cover - 抽象
        raise NotImplementedError


class FigureResult(Renderable):
    """包裹一个 Plotly figure（或 matplotlib 兜底 PNG base64）。"""

    def __init__(self, fig=None, png_base64=None, caption=""):
        self.fig = fig
        self.png_base64 = png_base64
        self.caption = caption

    def render(self, ctx):
        ctx.blocks.append({"kind": "figure", "fig": self.fig,
                           "png": self.png_base64, "caption": self.caption})


# ===========================================================================
# 运行上下文 / 结果块
# ===========================================================================
class RunContext:
    """单次 run() 的上下文：累积结果块 + 透传选项（绘图区间等）。"""

    def __init__(self, options):
        self.options = options or {}
        self.blocks = []  # 结果块列表

    # 选项读取（带默认值）
    def opt(self, key, default):
        value = self.options.get(key, default)
        return value if value is not None else default

    @property
    def theme(self):
        return str(self.opt("theme", "light") or "light").lower()

    @property
    def range_min(self):
        try:
            return float(self.opt("rangeMin", -10))
        except Exception:
            return -10.0

    @property
    def range_max(self):
        try:
            return float(self.opt("rangeMax", 10))
        except Exception:
            return 10.0

    # 追加块的便捷方法
    def latex(self, value, label=""):
        self.blocks.append({"kind": "latex", "tex": sp.latex(value), "label": label})

    def latex_raw(self, tex, label=""):
        self.blocks.append({"kind": "latex", "tex": tex, "label": label})

    def text(self, text):
        self.blocks.append({"kind": "text", "text": str(text)})

    def heading(self, text):
        self.blocks.append({"kind": "heading", "text": str(text)})


# ===========================================================================
# 解析层：LaTeX / 纯文本 → SymPy（兜底翻译迁移自旧 runtime，作为受限子路径）
# ===========================================================================
def _strip_math_wrappers(text: str) -> str:
    text = str(text or "").strip()
    text = re.sub(r"^\\begin\{equation\*?\}", "", text).strip()
    text = re.sub(r"\\end\{equation\*?\}$", "", text).strip()
    for opener, closer in (("$$", "$$"), ("\\[", "\\]"), ("\\(", "\\)"), ("$", "$")):
        if text.startswith(opener) and text.endswith(closer) and len(text) > len(opener) + len(closer) - 1:
            text = text[len(opener):len(text) - len(closer)].strip()
            break
    return text


def _latex_to_sympy_text(src: str) -> str:
    """把 LaTeX 文本尽力翻译成纯 SymPy 文本（不依赖 antlr 的兜底路径）。"""
    s = str(src or "").strip()
    # 去掉纯排版命令
    s = re.sub(r"\\(?:left|right|displaystyle|textstyle|limits|big|Big|bigg|Bigg|,|;|:|!|quad|qquad)", " ", s)
    s = re.sub(r"\\(?:mathrm|mathbf|mathit|mathcal|operatorname|text)\s*\{([^{}]*)\}", r"\1", s)
    # 分式（循环处理嵌套）
    for _ in range(8):
        nxt = re.sub(r"\\[dt]?frac\s*\{([^{}]+)\}\s*\{([^{}]+)\}", r"((\1)/(\2))", s)
        if nxt == s:
            break
        s = nxt
    # 根式
    s = re.sub(r"\\sqrt\s*\[([^\]]+)\]\s*\{([^{}]+)\}", r"((\2)**(1/(\1)))", s)
    s = re.sub(r"\\sqrt\s*\{([^{}]+)\}", r"sqrt(\1)", s)
    # 运算符
    s = re.sub(r"\\(?:cdot|times|ast)", "*", s)
    s = re.sub(r"\\div", "/", s)
    # 函数命令
    funcs = "sin|cos|tan|cot|sec|csc|sinh|cosh|tanh|arcsin|arccos|arctan|asin|acos|atan|exp|log|ln|abs"
    s = re.sub(r"\\(" + funcs + r")\s*\{([^{}]+)\}", r"\1(\2)", s)
    s = re.sub(r"\\(" + funcs + r")\s*\\([A-Za-z]+)", r"\1(\2)", s)
    s = re.sub(r"\\(" + funcs + r")(?![A-Za-z])", r"\1", s)
    # 上下标
    s = re.sub(r"\^\s*\{([^{}]+)\}", r"**(\1)", s)
    s = re.sub(r"_\s*\{([^{}]+)\}", r"_\1", s)
    # 其余命令（希腊字母等）去反斜杠保名字
    s = re.sub(r"\\([A-Za-z]+)", r"\1", s)
    # 铁律：清除残留裸反斜杠，杜绝 tokenizer 行连接符报错
    s = s.replace("\\", " ")
    return s.strip()


def _parse_latex_text(text: str):
    """把一段 LaTeX 解析为 SymPy 对象：antlr 优先，失败回退纯文本翻译。"""
    text = _strip_math_wrappers(text)
    if not text:
        raise GuaxiMathError("公式为空。")
    if "\\" in text and _ensure_latex_backend() and "\\partial" not in text:
        try:
            from sympy.parsing.latex import parse_latex
            return parse_latex(text)
        except Exception:
            pass
    plain = _latex_to_sympy_text(text).replace("^", "**")
    locs = _base_symbol_locals()
    if "=" in plain and not re.search(r"[<>!]=|==", plain):
        left, right = plain.split("=", 1)
        return sp.Eq(
            parse_expr(left, local_dict=locs, transformations=_TRANSFORMS),
            parse_expr(right, local_dict=locs, transformations=_TRANSFORMS),
        )
    return parse_expr(plain, local_dict=locs, transformations=_TRANSFORMS)


def _base_symbol_locals():
    """给纯文本兜底解析用的 local_dict：常用符号 + 希腊字母 + 常用函数。"""
    locs = {}
    for name in "x y z t u v w r s k m n p q a b c d".split():
        locs[name] = sp.Symbol(name)
    greek = ("alpha beta gamma delta epsilon zeta eta theta iota kappa lamda mu nu xi "
             "omicron pi rho sigma tau upsilon phi chi psi omega varepsilon vartheta varphi "
             "Gamma Delta Theta Lamda Xi Pi Sigma Upsilon Phi Psi Omega").split()
    for g in greek:
        locs.setdefault(g, sp.Symbol(g))
    locs.update({
        "pi": sp.pi, "E": sp.E, "I": sp.I, "oo": sp.oo, "infty": sp.oo,
        "sin": sp.sin, "cos": sp.cos, "tan": sp.tan, "cot": sp.cot, "sec": sp.sec, "csc": sp.csc,
        "asin": sp.asin, "acos": sp.acos, "atan": sp.atan, "atan2": sp.atan2,
        "sinh": sp.sinh, "cosh": sp.cosh, "tanh": sp.tanh,
        "exp": sp.exp, "log": sp.log, "ln": sp.log, "sqrt": sp.sqrt, "Abs": sp.Abs,
    })
    return locs


# ===========================================================================
# 数值求值（lambdify 包一层，统一处理复数 / 标量广播 / 异常）
# ===========================================================================
def _to_expr(value):
    """把命令参数尽量转为 SymPy 表达式。"""
    if isinstance(value, sp.Basic):
        return value
    if isinstance(value, str):
        return _parse_latex_text(value)
    try:
        return sp.sympify(value)
    except Exception as exc:
        raise GuaxiMathError(f"无法识别表达式：{value!r}（{exc}）")


def _eval_grid(expr, symbols, grids):
    fn = sp.lambdify(symbols, expr, "numpy")
    out = np.asarray(fn(*grids), dtype=np.complex128)
    if out.shape == ():
        out = np.full_like(np.asarray(grids[0], dtype=np.complex128), out, dtype=np.complex128)
    return out


def _free_symbols(value):
    if isinstance(value, (list, tuple, set)):
        acc = set()
        for item in value:
            acc |= _free_symbols(item)
        return acc
    return set(getattr(value, "free_symbols", set()) or set())


# ===========================================================================
# Plotly 绘图（交互）；不可用时回退 matplotlib 静态 PNG
# ===========================================================================
def _mpl_png_fallback(plot_calls, title=""):
    """plotly 缺失时的兜底：用 matplotlib 出 PNG（仅 1D 曲线）。"""
    import matplotlib
    matplotlib.use("Agg", force=True)
    import matplotlib.pyplot as plt
    plt.rcParams["font.sans-serif"] = ["Microsoft YaHei", "SimHei", "Noto Sans CJK SC", "DejaVu Sans"]
    plt.rcParams["axes.unicode_minus"] = False
    plt.figure(figsize=(8.8, 4.9))
    for xs, ys, label in plot_calls:
        plt.plot(xs, ys, label=label)
    plt.grid(True, alpha=0.28)
    plt.axhline(0, color="#888", linewidth=0.8)
    plt.axvline(0, color="#888", linewidth=0.8)
    if len(plot_calls) > 1:
        plt.legend(loc="best")
    if title:
        plt.title(title)
    plt.tight_layout()
    buf = io.BytesIO()
    plt.savefig(buf, format="png", dpi=150, bbox_inches="tight")
    plt.close("all")
    return base64.b64encode(buf.getvalue()).decode("ascii")


def _coerce_range(rng):
    """把 (sym, a, b) / (sym, a) 规整成 (Symbol, float, float)。"""
    if not isinstance(rng, (tuple, list)) or len(rng) < 2:
        raise GuaxiMathError("区间应写成 (变量, 起, 止)，例如 (x, -10, 10)。")
    sym = rng[0]
    if not isinstance(sym, sp.Symbol):
        sym = sp.Symbol(str(sym))
    if len(rng) >= 3:
        lo, hi = float(rng[1]), float(rng[2])
    else:
        lo, hi = -10.0, 10.0
    return sym, lo, hi


def _looks_like_range(value):
    return (isinstance(value, (tuple, list)) and len(value) in (2, 3)
            and isinstance(value[0], sp.Symbol))


def _build_plot(ctx, args, kwargs):
    """统一绘图入口：自动判定 1D 曲线 / 2D 隐式 / 3D 曲面。返回 FigureResult。"""
    exprs = []
    ranges = []
    for a in args:
        if _looks_like_range(a):
            ranges.append(_coerce_range(a))
        elif isinstance(a, (list, tuple)):
            exprs.extend(a)
        else:
            exprs.append(a)
    if not exprs:
        raise GuaxiMathError("plot 需要至少一个表达式，例如 plot(sin(x), (x,-10,10))。")
    exprs = [_to_expr(e) for e in exprs]

    # 推断变量与区间
    all_syms = sorted(_free_symbols(exprs), key=lambda s: str(s))
    if not ranges:
        lo, hi = ctx.range_min, ctx.range_max
        ranges = [(s, lo, hi) for s in (all_syms[:2] if all_syms else [sp.Symbol("x")])]
    range_syms = [r[0] for r in ranges]
    title = kwargs.get("title", "")

    # —— 3D 曲面：两个区间且无 Eq —— #
    if len(ranges) >= 2 and not any(isinstance(e, sp.Equality) for e in exprs):
        (xs_sym, x0, x1), (ys_sym, y0, y1) = ranges[0], ranges[1]
        xs = np.linspace(x0, x1, 80)
        ys = np.linspace(y0, y1, 80)
        X, Y = np.meshgrid(xs, ys)
        if not _HAS_PLOTLY:
            raise GuaxiMathError("绘制 3D 曲面需要 plotly，请先安装。")
        fig = go.Figure()
        for e in exprs:
            Z = np.real(_eval_grid(e, [xs_sym, ys_sym], [X, Y]))
            fig.add_trace(go.Surface(x=xs, y=ys, z=Z, name=sp.sstr(e), showscale=len(exprs) == 1))
        fig.update_layout(title=title or "三维曲面", scene=dict(
            xaxis_title=str(xs_sym), yaxis_title=str(ys_sym), zaxis_title="f"),
            margin=dict(l=0, r=0, t=40, b=0), height=520)
        return FigureResult(fig=fig, caption=f"三维曲面：{', '.join(str(s) for s in range_syms)}")

    # —— 2D 隐式等值线：含 Eq 或两区间 —— #
    if any(isinstance(e, sp.Equality) for e in exprs) and len(ranges) >= 2:
        (xs_sym, x0, x1), (ys_sym, y0, y1) = ranges[0], ranges[1]
        xs = np.linspace(x0, x1, 240)
        ys = np.linspace(y0, y1, 240)
        X, Y = np.meshgrid(xs, ys)
        if not _HAS_PLOTLY:
            raise GuaxiMathError("绘制隐式图需要 plotly，请先安装。")
        fig = go.Figure()
        for e in exprs:
            implicit = (e.lhs - e.rhs) if isinstance(e, sp.Equality) else e
            Z = np.real(_eval_grid(implicit, [xs_sym, ys_sym], [X, Y]))
            fig.add_trace(go.Contour(x=xs, y=ys, z=Z, contours=dict(start=0, end=0, coloring="none"),
                                     line=dict(width=2), showscale=False, name=sp.sstr(e)))
        fig.update_layout(title=title or "二维隐式图", xaxis_title=str(xs_sym),
                          yaxis_title=str(ys_sym), height=520)
        return FigureResult(fig=fig, caption="二维隐式图")

    # —— 1D 曲线（默认）—— #
    xs_sym, x0, x1 = ranges[0]
    xs = np.linspace(x0, x1, 700)
    plot_calls = []
    if _HAS_PLOTLY:
        fig = go.Figure()
    for e in exprs:
        vals = _eval_grid(e, [xs_sym], [xs])
        if np.iscomplexobj(vals) and np.nanmax(np.abs(np.imag(vals))) > 1e-9:
            if _HAS_PLOTLY:
                fig.add_trace(go.Scatter(x=xs, y=np.real(vals), mode="lines", name=f"Re {sp.sstr(e)}"))
                fig.add_trace(go.Scatter(x=xs, y=np.imag(vals), mode="lines", name=f"Im {sp.sstr(e)}"))
            else:
                plot_calls.append((xs, np.real(vals), f"Re {sp.sstr(e)}"))
                plot_calls.append((xs, np.imag(vals), f"Im {sp.sstr(e)}"))
        else:
            yv = np.real(vals)
            if _HAS_PLOTLY:
                fig.add_trace(go.Scatter(x=xs, y=yv, mode="lines", name=sp.sstr(e)))
            else:
                plot_calls.append((xs, yv, sp.sstr(e)))
    if not _HAS_PLOTLY:
        png = _mpl_png_fallback(plot_calls, title=title)
        return FigureResult(png_base64=png, caption=f"曲线图：{xs_sym} ∈ [{x0}, {x1}]")
    fig.update_layout(title=title or "函数图像", xaxis_title=str(xs_sym), yaxis_title="f",
                      height=460, hovermode="x unified")
    fig.add_hline(y=0, line_width=0.8, line_color="#888")
    return FigureResult(fig=fig, caption=f"曲线图：{xs_sym} ∈ [{x0}, {x1}]")


# ===========================================================================
# 富结果命令（自定义渲染）：特征分解 / 矩阵分解 / 统计分布
# ===========================================================================
class EigResult(Renderable):
    def __init__(self, matrix):
        self.matrix = matrix

    def render(self, ctx):
        M = self.matrix
        ctx.heading("特征值与特征向量")
        try:
            data = M.eigenvects()
        except Exception as exc:
            raise GuaxiMathError(f"特征分解失败：{exc}")
        for val, mult, vecs in data:
            ctx.latex_raw(f"\\lambda = {sp.latex(val)} \\quad (\\text{{重数}}\\ {mult})")
            for vec in vecs:
                ctx.latex_raw(f"v = {sp.latex(vec.T)}")


class DecompResult(Renderable):
    """LU / QR / SVD 等分解结果。"""

    def __init__(self, title, parts):
        self.title = title
        self.parts = parts  # [(label, value), ...]

    def render(self, ctx):
        ctx.heading(self.title)
        for label, value in self.parts:
            ctx.latex(value, label=label)


class DistResult(Renderable):
    """统计分布：PDF/PMF + CDF 交互图 + 期望/方差。"""

    def __init__(self, name, args, kwargs, discrete=False):
        self.name = name
        self.args = args
        self.kwargs = kwargs
        self.discrete = discrete

    def render(self, ctx):
        try:
            from scipy import stats as st
        except Exception as exc:
            raise GuaxiMathError(f"统计分布需要 scipy（{exc}）。请在 Python 运行时安装 scipy 后重试。")
        factory = getattr(st, self.name, None)
        if factory is None:
            raise GuaxiMathError(f"scipy.stats 中没有名为 `{self.name}` 的分布。")
        dist = factory(*self.args, **self.kwargs)
        discrete = isinstance(factory, st.rv_discrete) or self.discrete
        ctx.heading(f"统计分布：{self.name}({', '.join(str(a) for a in self.args)})")
        try:
            ctx.text(f"期望 E[X] = {float(dist.mean()):.4g}，方差 D[X] = {float(dist.var()):.4g}")
        except Exception:
            pass
        lo, hi = dist.ppf(0.001), dist.ppf(0.999)
        if not np.isfinite(lo):
            lo = ctx.range_min
        if not np.isfinite(hi):
            hi = ctx.range_max
        if _HAS_PLOTLY:
            fig = go.Figure()
            if discrete:
                ks = np.arange(int(np.floor(lo)), int(np.ceil(hi)) + 1)
                fig.add_trace(go.Bar(x=ks, y=dist.pmf(ks), name="PMF"))
                fig.add_trace(go.Scatter(x=ks, y=dist.cdf(ks), mode="lines+markers",
                                         name="CDF", yaxis="y2", line_shape="hv"))
            else:
                grid = np.linspace(lo, hi, 400)
                fig.add_trace(go.Scatter(x=grid, y=dist.pdf(grid), mode="lines", name="PDF", fill="tozeroy"))
                fig.add_trace(go.Scatter(x=grid, y=dist.cdf(grid), mode="lines", name="CDF", yaxis="y2"))
            fig.update_layout(title=f"{self.name} 分布", height=460,
                              yaxis=dict(title="PDF/PMF"),
                              yaxis2=dict(title="CDF", overlaying="y", side="right", range=[0, 1.02]))
            ctx.blocks.append({"kind": "figure", "fig": fig, "png": None, "caption": ""})
        else:
            ctx.text("（plotly 不可用，已跳过分布图）")


# ===========================================================================
# 命令库 —— 阶段 1：高数 / 线代 / 概率统计
# 每个学科 = 往这里加函数；命令通过 build_namespace 注入受控命名空间。
# ===========================================================================
def _build_commands(ctx):
    """构造命令字典。命令返回 SymPy 对象（自动 LaTeX）或 Renderable（自渲染）。"""

    def _sym(name_or_sym):
        if isinstance(name_or_sym, sp.Symbol):
            return name_or_sym
        return sp.Symbol(str(name_or_sym))

    def _matrix(rows):
        try:
            return sp.Matrix(rows)
        except Exception as exc:
            raise GuaxiMathError(f"无法构造矩阵：{exc}")

    # ---- 微积分 ----
    def limit(expr, var=None, point=0, dir="+-"):
        expr = _to_expr(expr)
        var = _sym(var) if var is not None else (sorted(_free_symbols(expr), key=str) or [sp.Symbol("x")])[0]
        try:
            return sp.limit(expr, var, sp.sympify(point), dir if dir in ("+", "-") else "+-") \
                if dir in ("+", "-") else sp.limit(expr, var, sp.sympify(point))
        except Exception as exc:
            raise GuaxiMathError(f"求极限失败：{exc}")

    def diff(expr, var=None, n=1):
        expr = _to_expr(expr)
        var = _sym(var) if var is not None else (sorted(_free_symbols(expr), key=str) or [sp.Symbol("x")])[0]
        return sp.diff(expr, var, int(n))

    def integrate(expr, var=None, a=None, b=None):
        expr = _to_expr(expr)
        # 支持三种写法：integrate(f, x) / integrate(f, x, a, b) / integrate(f, (x, a, b))
        if isinstance(var, (tuple, list)):
            sym = _sym(var[0])
            if len(var) >= 3:
                return sp.integrate(expr, (sym, sp.sympify(var[1]), sp.sympify(var[2])))
            return sp.integrate(expr, sym)
        var = _sym(var) if var is not None else (sorted(_free_symbols(expr), key=str) or [sp.Symbol("x")])[0]
        if a is not None and b is not None:
            return sp.integrate(expr, (var, sp.sympify(a), sp.sympify(b)))
        return sp.integrate(expr, var)

    def series(expr, var=None, x0=0, n=6):
        expr = _to_expr(expr)
        var = _sym(var) if var is not None else (sorted(_free_symbols(expr), key=str) or [sp.Symbol("x")])[0]
        return sp.series(expr, var, sp.sympify(x0), int(n))

    def taylor(expr, var=None, x0=0, n=6):
        return series(expr, var, x0, n).removeO()

    def simplify(expr):
        return sp.simplify(_to_expr(expr))

    def expand(expr):
        return sp.expand(_to_expr(expr))

    def factor(expr):
        return sp.factor(_to_expr(expr))

    def solve(*eqs, **kwargs):
        parsed = [_to_expr(e) for e in eqs]
        target = parsed[0] if len(parsed) == 1 else parsed
        syms = kwargs.get("vars")
        if syms is not None and not isinstance(syms, (list, tuple)):
            syms = [syms]
        try:
            if syms:
                return sp.solve(target, [_sym(s) for s in syms], dict=True)
            return sp.solve(target, dict=True)
        except Exception as exc:
            raise GuaxiMathError(f"求解失败：{exc}")

    def dsolve(eq, func=None):
        eq = _to_expr(eq)
        try:
            return sp.dsolve(eq, func) if func is not None else sp.dsolve(eq)
        except Exception as exc:
            raise GuaxiMathError(f"微分方程求解失败：{exc}")

    def summation(expr, var, a, b):
        return sp.summation(_to_expr(expr), (_sym(var), sp.sympify(a), sp.sympify(b)))

    def product(expr, var, a, b):
        return sp.product(_to_expr(expr), (_sym(var), sp.sympify(a), sp.sympify(b)))

    # ---- 线性代数 ----
    def matrix(rows):
        return _matrix(rows)

    def det(M):
        return _matrix(M).det() if not isinstance(M, sp.MatrixBase) else M.det()

    def inv(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        try:
            return M.inv()
        except Exception as exc:
            raise GuaxiMathError(f"求逆失败：{exc}")

    def rref(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return M.rref()[0]

    def rank(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return M.rank()

    def transpose(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return M.T

    def eig(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return EigResult(M)

    def eigenvalues(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return sp.Matrix(list(M.eigenvals().keys()))

    def charpoly(M, var="lamda"):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        return M.charpoly(_sym(var)).as_expr()

    def nullspace(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        vecs = M.nullspace()
        return DecompResult("零空间（基）", [("", v) for v in vecs]) if vecs else GuaxiMathError("零空间只含零向量。")

    def lu(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        L, U, _perm = M.LUdecomposition()
        return DecompResult("LU 分解", [("L =", L), ("U =", U)])

    def qr(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        Q, R = M.QRdecomposition()
        return DecompResult("QR 分解", [("Q =", Q), ("R =", R)])

    def svd(M):
        M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
        # 符号 SVD 较难，这里走数值 SVD（numpy）。
        try:
            A = np.array(M.evalf().tolist(), dtype=float)
        except Exception as exc:
            raise GuaxiMathError(f"SVD 需要数值矩阵：{exc}")
        U, s, Vt = np.linalg.svd(A)
        return DecompResult("奇异值分解（数值）", [
            ("U =", sp.Matrix(np.round(U, 6))),
            ("\\Sigma\\ (奇异值) =", sp.Matrix(np.round(s, 6))),
            ("V^T =", sp.Matrix(np.round(Vt, 6))),
        ])

    def linsolve(M, b=None):
        if b is not None:
            M = M if isinstance(M, sp.MatrixBase) else _matrix(M)
            b = b if isinstance(b, sp.MatrixBase) else _matrix(b)
            return sp.linsolve((M, b))
        return sp.linsolve(_to_expr(M) if not isinstance(M, (list, tuple)) else [_to_expr(e) for e in M])

    # ---- 概率统计 ----
    def _make_dist(scipy_name, discrete=False):
        def factory(*args, **kwargs):
            return DistResult(scipy_name, args, kwargs, discrete=discrete)
        return factory

    def expectation(dist_or_expr, *args, **kwargs):
        if isinstance(dist_or_expr, DistResult):
            from scipy import stats as st
            d = getattr(st, dist_or_expr.name)(*dist_or_expr.args, **dist_or_expr.kwargs)
            return sp.Float(float(d.mean()))
        raise GuaxiMathError("expectation 目前接收分布对象，如 expectation(Normal(0,1))。")

    def variance(dist_or_expr):
        if isinstance(dist_or_expr, DistResult):
            from scipy import stats as st
            d = getattr(st, dist_or_expr.name)(*dist_or_expr.args, **dist_or_expr.kwargs)
            return sp.Float(float(d.var()))
        raise GuaxiMathError("variance 目前接收分布对象，如 variance(Normal(0,1))。")

    def plot(*args, **kwargs):
        return _build_plot(ctx, args, kwargs)

    def tex(s):
        """把 LaTeX 字符串解析为 SymPy 对象，例如 tex(r'\\frac{a}{b}')。"""
        return _parse_latex_text(str(s))

    def N(expr, n=15):
        return sp.N(_to_expr(expr), int(n))

    commands = {
        # 微积分
        "limit": limit, "diff": diff, "integrate": integrate, "series": series, "taylor": taylor,
        "simplify": simplify, "expand": expand, "factor": factor, "solve": solve, "dsolve": dsolve,
        "summation": summation, "product": product, "Sum": summation,
        # 线代
        "matrix": matrix, "Matrix": sp.Matrix, "det": det, "inv": inv, "rref": rref, "rank": rank,
        "transpose": transpose, "eig": eig, "eigenvalues": eigenvalues, "charpoly": charpoly,
        "nullspace": nullspace, "lu": lu, "qr": qr, "svd": svd, "linsolve": linsolve,
        "eye": sp.eye, "zeros": sp.zeros, "ones": sp.ones, "diag": sp.diag,
        # 概率统计（首字母大写，避免与 sympy gamma 等函数名冲突）
        "Normal": _make_dist("norm"), "Uniform": _make_dist("uniform"),
        "Exponential": _make_dist("expon"), "Gamma": _make_dist("gamma"),
        "Beta": _make_dist("beta"), "ChiSquared": _make_dist("chi2"), "StudentT": _make_dist("t"),
        "Binomial": _make_dist("binom", discrete=True), "Poisson": _make_dist("poisson", discrete=True),
        "Bernoulli": _make_dist("bernoulli", discrete=True), "Geometric": _make_dist("geom", discrete=True),
        "expectation": expectation, "variance": variance,
        # 绘图与杂项
        "plot": plot, "tex": tex, "N": N,
    }
    return commands


# ===========================================================================
# 命名空间构造 + 自动符号注入
# ===========================================================================
def _constants():
    return {
        "pi": sp.pi, "E": sp.E, "e": sp.E, "I": sp.I, "oo": sp.oo, "infty": sp.oo,
        "sin": sp.sin, "cos": sp.cos, "tan": sp.tan, "cot": sp.cot, "sec": sp.sec, "csc": sp.csc,
        "asin": sp.asin, "acos": sp.acos, "atan": sp.atan, "atan2": sp.atan2,
        "sinh": sp.sinh, "cosh": sp.cosh, "tanh": sp.tanh,
        "asinh": sp.asinh, "acosh": sp.acosh, "atanh": sp.atanh,
        "exp": sp.exp, "log": sp.log, "ln": sp.log, "sqrt": sp.sqrt, "Abs": sp.Abs, "abs": sp.Abs,
        "sign": sp.sign, "floor": sp.floor, "ceiling": sp.ceiling, "factorial": sp.factorial,
        "gamma": sp.gamma, "erf": sp.erf, "Min": sp.Min, "Max": sp.Max, "re": sp.re, "im": sp.im,
        "conjugate": sp.conjugate, "arg": sp.arg, "Rational": sp.Rational, "Eq": sp.Eq,
        "Derivative": sp.Derivative, "Integral": sp.Integral, "Function": sp.Function,
        "DiracDelta": sp.DiracDelta, "Heaviside": sp.Heaviside, "true": True, "false": False,
    }


_SAFE_BUILTINS = {
    "abs": abs, "min": min, "max": max, "sum": sum, "len": len, "range": range,
    "list": list, "tuple": tuple, "dict": dict, "set": set, "float": float, "int": int,
    "complex": complex, "str": str, "bool": bool, "enumerate": enumerate, "zip": zip,
    "map": map, "filter": filter, "sorted": sorted, "round": round, "print": print,
    "True": True, "False": False, "None": None,
}


def _collect_free_names(tree):
    """收集需要自动符号化的自由名字，以及被当作函数调用的未知名字。"""
    assigned = set()
    used = set()
    called = set()
    for node in ast.walk(tree):
        if isinstance(node, (ast.Assign,)):
            for t in node.targets:
                for n in ast.walk(t):
                    if isinstance(n, ast.Name):
                        assigned.add(n.id)
        elif isinstance(node, (ast.AugAssign, ast.AnnAssign)):
            tgt = node.target
            if isinstance(tgt, ast.Name):
                assigned.add(tgt.id)
        elif isinstance(node, (ast.For, ast.comprehension)):
            tgt = getattr(node, "target", None)
            if tgt is not None:
                for n in ast.walk(tgt):
                    if isinstance(n, ast.Name):
                        assigned.add(n.id)
        elif isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
            called.add(node.func.id)
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
            used.add(node.id)
    return used, assigned, called


def build_namespace(ctx, workspace):
    ns = {}
    ns.update(_constants())
    ns.update(_build_commands(ctx))
    ns["__builtins__"] = dict(_SAFE_BUILTINS)
    # 恢复用户在工作区里定义过的变量
    ns.update(workspace or {})
    return ns


def inject_auto_symbols(tree, ns):
    used, assigned, called = _collect_free_names(tree)
    reserved = set(ns.keys()) | set(_SAFE_BUILTINS.keys()) | {"__builtins__"}
    for name in used:
        if name in reserved or name in assigned or name in ns:
            continue
        # 被当作函数调用的未知名字 → 未知函数（便于写 y(x) 之类微分方程）
        if name in called:
            ns[name] = sp.Function(name)
        else:
            ns[name] = sp.Symbol(name)


# ===========================================================================
# 逐语句执行 + 交互式显示
# ===========================================================================
# 命令 → 结果中文标题：让每条结果都带说明，而不是只有一个孤零零的公式。
_COMMAND_LABELS = {
    "limit": "极限", "diff": "导数", "integrate": "积分结果",
    "series": "级数展开", "taylor": "泰勒展开", "summation": "求和", "Sum": "求和",
    "product": "连乘积", "simplify": "化简结果", "expand": "展开", "factor": "因式分解",
    "solve": "方程的解", "dsolve": "微分方程的解", "linsolve": "线性方程组的解",
    "det": "行列式", "inv": "逆矩阵", "rref": "行最简形 (RREF)", "rank": "秩",
    "transpose": "转置矩阵", "charpoly": "特征多项式", "eigenvalues": "特征值",
    "expectation": "期望 E[X]", "variance": "方差 D[X]", "N": "数值结果",
    "tex": "表达式", "factorial": "阶乘",
}


def _label_for_node(value_node):
    """从 AST 表达式节点推断结果标题：命令调用用其中文名，普通表达式用「结果」。"""
    if isinstance(value_node, ast.Call) and isinstance(value_node.func, ast.Name):
        return _COMMAND_LABELS.get(value_node.func.id, "结果")
    return "结果"


def _is_displayable_math(value):
    return isinstance(value, (sp.Basic, sp.MatrixBase)) or isinstance(value, (int, float, complex))


def _record_value(ctx, value, label=""):
    if value is None:
        return
    if isinstance(value, Renderable):
        # 富结果对象（特征分解 / 分布 / 绘图）自带标题，外层 label 交给它忽略。
        value.render(ctx)
    elif isinstance(value, GuaxiMathError):
        ctx.text(f"⚠ {value}")
    elif _is_displayable_math(value):
        ctx.latex(value if isinstance(value, (sp.Basic, sp.MatrixBase)) else sp.sympify(value), label=label)
    elif isinstance(value, (list, tuple)):
        # 解的列表等：先给一个标题，再逐个显示。
        if value and all(_is_displayable_math(v) or isinstance(v, dict) for v in value):
            if label:
                ctx.heading(label)
            for v in value:
                _record_value(ctx, v)
        else:
            ctx.text(repr(value))
    elif isinstance(value, dict):
        # solve(dict=True) 的解
        if label:
            ctx.heading(label)
        parts = ", ".join(f"{sp.latex(k)} = {sp.latex(v)}" for k, v in value.items())
        ctx.latex_raw(parts if parts else "\\{\\}")
    else:
        ctx.text(repr(value))


def _exec_one_node(node, ns, ctx):
    """执行单个顶层语句并记录其显示结果（异常向上抛，由调用方逐条兜底）。"""
    if isinstance(node, ast.Expr):
        code = compile(ast.Expression(node.value), "<guaximath>", "eval")
        value = eval(code, ns)  # noqa: S307 - 受控命名空间 + 自动符号
        _record_value(ctx, value, label=_label_for_node(node.value))
    else:
        mod = ast.Module(body=[node], type_ignores=[])
        code = compile(mod, "<guaximath>", "exec")
        exec(code, ns)  # noqa: S102 - 受控命名空间
        # 赋值语句：显示 name = 值
        if isinstance(node, ast.Assign):
            for t in node.targets:
                if isinstance(t, ast.Name) and t.id in ns:
                    val = ns[t.id]
                    if isinstance(val, (sp.Basic, sp.MatrixBase)):
                        ctx.latex_raw(f"{sp.latex(sp.Symbol(t.id))} = {sp.latex(val)}")
                    elif isinstance(val, (int, float, complex)):
                        ctx.latex_raw(f"{sp.latex(sp.Symbol(t.id))} = {sp.latex(sp.sympify(val))}")
                    else:
                        ctx.text(f"{t.id} 已存储")


def _exec_node_list(nodes, ns, ctx):
    """逐条执行顶层语句；单条出错只提示该条，不影响其余。"""
    for node in nodes:
        try:
            _exec_one_node(node, ns, ctx)
        except GuaxiMathError as exc:
            ctx.text(f"⚠ {exc}")
        except Exception as exc:  # noqa: BLE001
            ctx.text(f"⚠ 出错：{type(exc).__name__}: {exc}")


def _expr_locals(ns):
    """给 parse_expr 兜底用的符号/函数字典（不含 Python 可调用命令）。"""
    locs = _base_symbol_locals()
    for name, val in ns.items():
        if isinstance(val, (sp.Basic, sp.MatrixBase)) or isinstance(val, sp.FunctionClass):
            locs[name] = val
    return locs


def _exec_line_by_line(source, ns, ctx):
    """整段不是合法 Python 时的逐行兜底：Python → 隐式乘法表达式 → LaTeX。

    支持数学写法（如 2x、3sin(x)），并让坏行不拖垮整格。
    """
    for raw in source.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        # 1) 先当合法 Python 语句
        try:
            sub = ast.parse(line, mode="exec")
            inject_auto_symbols(sub, ns)
            _exec_node_list(sub.body, ns, ctx)
            continue
        except SyntaxError:
            pass
        # 2) 含反斜杠 / $ → 走 LaTeX 解析
        if "\\" in line or line.startswith("$"):
            try:
                _record_value(ctx, _parse_latex_text(line), label="公式")
            except Exception as exc:  # noqa: BLE001
                ctx.text(f"⚠ 无法解析：{line}（{exc}）")
            continue
        # 3) 纯数学表达式（隐式乘法，如 2x + 1）
        try:
            expr = parse_expr(line.replace("^", "**"), local_dict=_expr_locals(ns),
                              transformations=_TRANSFORMS)
            _record_value(ctx, expr, label="结果")
        except Exception:  # noqa: BLE001
            ctx.text(f"⚠ 无法解析：{line}")


def _persist_workspace(ns, workspace):
    reserved = set(_constants().keys()) | set(_SAFE_BUILTINS.keys()) | {"__builtins__"}
    cmd_names = set(_build_commands(RunContext({})).keys())
    reserved |= cmd_names
    for name, val in ns.items():
        if name in reserved or name.startswith("__"):
            continue
        workspace[name] = val


# ===========================================================================
# 渲染层：结果块 → display 字典
# ===========================================================================
def _render_markdown(blocks):
    """符号 / 文字结果合成 markdown，交给 App 内置 MathJax 渲染（矩阵 OK，且跟随暗色主题）。"""
    lines = []
    for b in blocks:
        kind = b["kind"]
        if kind == "heading":
            lines.append(f"### {b['text']}")
        elif kind == "text":
            lines.append(b["text"])
        elif kind == "latex":
            label = b.get("label") or ""
            if label:
                lines.append(f"**{label}**")
            lines.append(f"$$\n{b['tex']}\n$$")
        elif kind == "figure" and b.get("caption"):
            lines.append(f"*{b['caption']}*")
    return {"type": "markdown", "mime": "text/markdown", "markdown": "\n\n".join(lines)}


def _render_figure_html(block, theme="light"):
    """单张图渲染为独立 html display（交互 Plotly，自包含 plotly.js）。

    设计说明：Plotly 图固定使用亮色主题——它在独立 iframe 里渲染，与 App 主题解耦，
    保持浅色背景以获得稳定、清晰的绘图观感（theme 参数保留以备将来需要）。
    """
    fig = block.get("fig")
    if fig is not None and _HAS_PLOTLY:
        html = pio.to_html(fig, include_plotlyjs=True, full_html=False, config={"responsive": True})
        return {"type": "html", "mime": "text/html", "html": html}
    if block.get("png"):
        return {"type": "image", "mime": "image/png", "data_base64": block["png"]}
    return {"type": "markdown", "mime": "text/markdown", "markdown": "（图像不可用）"}


def _render(ctx):
    """把结果块按「图」切分成若干 display：

      - 连续的符号 / 文字块 → 合成一个 markdown display（走 App MathJax，跟随主题）；
      - 每张图 → 单独一个 html display（交互 Plotly，主题独立）。

    返回单个 display 字典（仅一段时，向后兼容）或 display 字典列表（多段时）。
    这样既修复了「矩阵公式渲染不了」（交给 MathJax），也修复了「非绘图区不跟随暗色」。
    """
    blocks = ctx.blocks
    if not blocks:
        return {"type": "markdown", "mime": "text/markdown", "markdown": "（无输出）"}
    segments = []  # [('md', [blocks]) | ('fig', block)]
    buf = []
    for b in blocks:
        if b["kind"] == "figure":
            if buf:
                segments.append(("md", buf))
                buf = []
            segments.append(("fig", b))
        else:
            buf.append(b)
    if buf:
        segments.append(("md", buf))
    displays = []
    for kind, payload in segments:
        if kind == "md":
            displays.append(_render_markdown(payload))
        else:
            displays.append(_render_figure_html(payload, ctx.theme))
    return displays[0] if len(displays) == 1 else displays


# ===========================================================================
# 旧内容兼容：整段是裸 LaTeX（不是合法 Python）时的兜底
# ===========================================================================
def _looks_like_raw_latex(source):
    s = source.strip()
    return ("\\" in s or s.startswith("$")) and "\n" not in s.strip()


def _run_latex_fallback(source, ctx):
    try:
        expr = _parse_latex_text(source)
    except GuaxiMathError as exc:
        ctx.text(f"⚠ {exc}")
        return
    ctx.latex(expr, label="公式：")
    try:
        ctx.latex(sp.simplify(expr), label="化简：")
    except Exception:
        pass


# ===========================================================================
# 主入口
# ===========================================================================
def run(source, options=None, ns=None):
    """执行一段数学工作区源码，返回 display 字典。

    参数：
      source  : 数学块的多行文本（命令/赋值/裸表达式）。
      options : 透传选项（rangeMin/rangeMax 等）。
      ns      : 会话全局字典；工作区变量持久化在 ns['__GUAXIMATH_WS__'] 里。
    """
    options = options or {}
    ns = ns if isinstance(ns, dict) else {}
    workspace = ns.setdefault("__GUAXIMATH_WS__", {})
    ctx = RunContext(options)
    source = str(source or "")
    if not source.strip():
        return {"type": "markdown", "mime": "text/markdown", "markdown": "（数学块为空）"}

    namespace = build_namespace(ctx, workspace)
    # 主路径：整段是合法 Python/SymPy → 逐顶层语句执行（保证多行语句正确分组）。
    # 兜底：整段不是合法 Python（如含裸 LaTeX 或数学写法 2x）→ 逐行解析。
    try:
        tree = ast.parse(source, mode="exec")
    except SyntaxError:
        tree = None
    try:
        if tree is not None:
            inject_auto_symbols(tree, namespace)
            _exec_node_list(tree.body, namespace, ctx)
        elif _looks_like_raw_latex(source):
            _run_latex_fallback(source, ctx)
        else:
            _exec_line_by_line(source, namespace, ctx)
    except Exception as exc:  # noqa: BLE001 - 统一兜底成中文提示
        ctx.text(f"⚠ 运行出错：{type(exc).__name__}: {exc}")

    try:
        _persist_workspace(namespace, workspace)
    except Exception:
        pass
    return _render(ctx)
