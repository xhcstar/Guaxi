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

ENGINE_VERSION = "1.1.0"

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
        # 会话引用与「重型对象登记表」（carrier）。run() 注入；脱离会话时为 None（如 _persist_workspace 探测）。
        # 重型对象（数值解迭代历史、仿真轨迹）登记在会话 globals 的 __GUAXIMATH_CARRIERS__ 里，
        # 供右侧面板 / 独立工作台读取——不进 display 字典，不改 display 协议。
        self.session = None
        self.carriers = None

    def add_carrier(self, kind, command, label="", params=None, summary="", solution_var=""):
        """登记一个可在面板/工作台里复跑或回放的重型结果。值必须 JSON 安全（前端要读）。

        返回 carrier key（无会话时返回空串、不报错）。
        """
        if self.carriers is None or self.session is None:
            return ""
        seq = int(self.session.get("__GUAXIMATH_CARRIER_SEQ__", 0)) + 1
        self.session["__GUAXIMATH_CARRIER_SEQ__"] = seq
        key = f"{command}_{seq}"
        self.carriers[key] = {
            "key": key,
            "kind": str(kind or "numeric"),
            "command": str(command or ""),
            "label": str(label or command or ""),
            "params": params if isinstance(params, dict) else {},
            "summary": str(summary or ""),
            "solutionVar": str(solution_var or ""),
            "seq": seq,
        }
        return key

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


# ---------------------------------------------------------------------------
# 模块级共享 helper（供阶段二/三/四各学科命令复用，避免与阶段一闭包重复）
# ---------------------------------------------------------------------------
def _as_sym(name_or_sym):
    """把名字/符号统一成 sp.Symbol。"""
    if isinstance(name_or_sym, sp.Symbol):
        return name_or_sym
    return sp.Symbol(str(name_or_sym))


def _as_matrix(rows):
    """把列表 / 矩阵统一成 sp.Matrix（已是矩阵则原样返回）。"""
    if isinstance(rows, sp.MatrixBase):
        return rows
    try:
        return sp.Matrix(rows)
    except Exception as exc:  # noqa: BLE001
        raise GuaxiMathError(f"无法构造矩阵：{exc}")


def _coords_list(coords):
    """坐标参数（名字/符号的列表）统一成 [Symbol, ...]。"""
    if isinstance(coords, sp.MatrixBase):
        coords = list(coords)
    if not isinstance(coords, (list, tuple)) or not coords:
        raise GuaxiMathError("请提供坐标列表，例如 [r, theta]。")
    return [_as_sym(c) for c in coords]


def _pick_var(expr, preferred="x"):
    """从表达式自由符号里挑一个变量：优先 preferred，否则取字典序首个，再否则造一个。"""
    syms = sorted(_free_symbols(expr), key=str)
    for s in syms:
        if str(s) == preferred:
            return s
    if syms:
        return syms[0]
    return sp.Symbol(preferred)


def _as_vec(value):
    """把 3 维（或任意维）向量参数统一成列向量 Matrix（支持 list / Matrix）。"""
    if isinstance(value, sp.MatrixBase):
        return value if value.shape[1] == 1 else value.T
    if isinstance(value, (list, tuple)):
        return sp.Matrix(list(value))
    raise GuaxiMathError("请提供向量，例如 [cos(u)*sin(v), sin(u)*sin(v), cos(v)]。")


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


class SeriesResult(Renderable):
    """洛朗 / 幂级数展开 + 可选留数（复变阶段）。"""

    def __init__(self, series_expr, residue=None, title="级数展开"):
        self.series_expr = series_expr
        self.residue = residue
        self.title = title

    def render(self, ctx):
        ctx.heading(self.title)
        ctx.latex(self.series_expr)
        if self.residue is not None:
            ctx.latex_raw(f"\\operatorname{{Res}} = {sp.latex(self.residue)}")


class TensorResult(Renderable):
    """带标签的指标分量集合（Christoffel / Riemann / Ricci 等张量阶段）。

    components: dict[tuple[int,...], value]，只收非零分量。
    coords:     坐标符号列表，用于把下标渲染成坐标名（Γ^{r}_{θθ} 等）。
    fmt:        (idx, coord_latex_names) -> str，返回左端 LaTeX；缺省给 T_{...}。
    """

    def __init__(self, title, components, coords, fmt=None):
        self.title = title
        self.components = components
        self.coords = list(coords or [])
        self.fmt = fmt

    def render(self, ctx):
        ctx.heading(self.title)
        if not self.components:
            ctx.text("所有分量均为 0。")
            return
        names = [sp.latex(c) for c in self.coords]
        for idx, val in self.components.items():
            if self.fmt is not None:
                lhs = self.fmt(idx, names)
            else:
                lhs = "T_{" + "".join(names[i] for i in idx) + "}"
            ctx.latex_raw(f"{lhs} = {sp.latex(val)}")


class NumericResult(Renderable):
    """数值计算结果：数值 + 收敛历史表 + 可选残差-迭代图（数值阶段 / 仿真阶段共用）。

    value      : SymPy 标量/矩阵 或 数值。
    iterations : [{"n":int, "x":..., "residual":float}, ...]，可空。
    """

    def __init__(self, value, iterations=None, method="", tol=None,
                 converged=None, title="数值结果", note=""):
        self.value = value
        self.iterations = iterations or []
        self.method = method
        self.tol = tol
        self.converged = converged
        self.title = title
        self.note = note

    def render(self, ctx):
        ctx.heading(self.title)
        if isinstance(self.value, (sp.Basic, sp.MatrixBase)):
            ctx.latex(self.value)
        elif self.value is not None:
            ctx.latex(sp.sympify(self.value))
        status = []
        if self.method:
            status.append(f"方法：{self.method}")
        if self.converged is not None:
            status.append("已收敛" if self.converged else "未收敛")
        if self.tol is not None:
            status.append(f"容差 {self.tol:g}")
        if self.note:
            status.append(self.note)
        if status:
            ctx.text("（" + "，".join(status) + "）")
        # 收敛历史 → markdown 表格（走 MathJax/markdown 文本块）。
        if self.iterations:
            rows = ["| n | x | 残差 |", "| --- | --- | --- |"]
            for it in self.iterations[:50]:
                xv = it.get("x")
                if isinstance(xv, (list, tuple)):
                    xs = "(" + ", ".join(f"{float(v):.6g}" for v in xv) + ")"
                else:
                    try:
                        xs = f"{float(xv):.6g}"
                    except Exception:  # noqa: BLE001
                        xs = str(xv)
                rows.append(f"| {it.get('n')} | {xs} | {float(it.get('residual', 0)):.3e} |")
            ctx.text("\n".join(rows))
            # 残差-迭代图（对数 y），plotly 可用时给一张交互图。
            if _HAS_PLOTLY and len(self.iterations) > 1:
                ns_ = [it.get("n") for it in self.iterations]
                rs_ = [max(float(it.get("residual", 0)), 1e-300) for it in self.iterations]
                fig = go.Figure()
                fig.add_trace(go.Scatter(x=ns_, y=rs_, mode="lines+markers", name="残差"))
                fig.update_layout(title="收敛过程（残差 vs 迭代）", height=380,
                                  xaxis_title="迭代 n", yaxis_title="残差",
                                  yaxis_type="log")
                ctx.blocks.append({"kind": "figure", "fig": fig, "png": None, "caption": ""})


class Simulator:
    """仿真器扩展基类（轻量占位 / 明确扩展缝）。

    子类实现 derivatives(t, state) -> list 即可接入 odeint / phase_portrait；
    后续可在此基础上扩展为带媒体回放的重型仿真（接 guaxi_python_worker.py 的媒体产出）。
    本轮不强制使用——内置 odeint/phase_portrait 直接吃表达式，Simulator 仅为后续扩展预留。
    """

    name = "simulator"

    def derivatives(self, t, state):  # pragma: no cover - 抽象
        raise NotImplementedError


# ===========================================================================
# 命令库 —— 阶段 1：高数 / 线代 / 概率统计
# 每个学科 = 往这里加函数；命令通过 build_namespace 注入受控命名空间。
# 阶段 2/3/4 的学科命令拆到本文件后面的 _commands_<subject>(ctx) 里，
# 由 _build_commands 末尾统一 merge（保持单文件、按学科分节）。
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
    # 阶段 2/3/4：按学科分节 merge（每个 _commands_<subject> 返回一个命令字典）。
    for _group in (
        _commands_complex,
        _commands_physics,
        _commands_geometry,
        _commands_matrixtheory,
        _commands_functional,
        _commands_numerical,
        _commands_sim,
    ):
        commands.update(_group(ctx))
    return commands


# ===========================================================================
# 命令库 —— 阶段 2a：复变函数（留数 / 洛朗级数 / 共形映射）
# ===========================================================================
def _commands_complex(ctx):
    def residue(expr, z=None, z0=0):
        expr = _to_expr(expr)
        z = _as_sym(z) if z is not None else _pick_var(expr, "z")
        try:
            return sp.residue(expr, z, sp.sympify(z0))
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"求留数失败：{exc}")

    def laurent(expr, z=None, z0=0, n=6):
        expr = _to_expr(expr)
        z = _as_sym(z) if z is not None else _pick_var(expr, "z")
        try:
            ser = sp.series(expr, z, sp.sympify(z0), int(n))
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"洛朗/级数展开失败：{exc}")
        try:
            res = sp.residue(expr, z, sp.sympify(z0))
        except Exception:  # noqa: BLE001
            res = None
        return SeriesResult(ser, residue=res, title="洛朗展开")

    def contour_integral(expr, z=None, poles=None):
        expr = _to_expr(expr)
        z = _as_sym(z) if z is not None else _pick_var(expr, "z")
        if not poles:
            raise GuaxiMathError("请提供围道内的极点列表，例如 contour_integral(f, z, poles=[0, I])。")
        if not isinstance(poles, (list, tuple)):
            poles = [poles]
        total = sum(sp.residue(expr, z, sp.sympify(p)) for p in poles)
        return 2 * sp.pi * sp.I * total

    def conformal(expr, z=None):
        expr = _to_expr(expr)
        z = _as_sym(z) if z is not None else _pick_var(expr, "z")
        # 共形映射在 f'(z)≠0 处保角，这里给出导数（结果块自带「共形映射 f'」标题）。
        return sp.diff(expr, z)

    def cauchy_riemann(u, v, x=None, y=None):
        u = _to_expr(u)
        v = _to_expr(v)
        x = _as_sym(x) if x is not None else _pick_var(u, "x")
        y = _as_sym(y) if y is not None else _pick_var(u, "y")
        return [
            sp.Eq(sp.diff(u, x), sp.diff(v, y)),
            sp.Eq(sp.diff(u, y), -sp.diff(v, x)),
        ]

    return {
        "residue": residue, "laurent": laurent, "contour_integral": contour_integral,
        "conformal": conformal, "cauchy_riemann": cauchy_riemann,
    }


# ===========================================================================
# 命令库 —— 阶段 2b：数学物理方法（积分变换 / 特殊函数 / 分离变量 / 格林函数）
# ===========================================================================
def _commands_physics(ctx):
    def laplace(expr, t=None, s=None):
        expr = _to_expr(expr)
        t = _as_sym(t) if t is not None else _pick_var(expr, "t")
        s = _as_sym(s) if s is not None else sp.Symbol("s")
        try:
            return sp.laplace_transform(expr, t, s, noconds=True)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"拉普拉斯变换失败：{exc}")

    def inverse_laplace(expr, s=None, t=None):
        expr = _to_expr(expr)
        s = _as_sym(s) if s is not None else _pick_var(expr, "s")
        t = _as_sym(t) if t is not None else sp.Symbol("t")
        try:
            return sp.inverse_laplace_transform(expr, s, t, noconds=True)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"拉普拉斯逆变换失败：{exc}")

    def fourier(expr, x=None, k=None):
        expr = _to_expr(expr)
        x = _as_sym(x) if x is not None else _pick_var(expr, "x")
        k = _as_sym(k) if k is not None else sp.Symbol("k")
        try:
            return sp.fourier_transform(expr, x, k)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"傅里叶变换失败：{exc}")

    def inverse_fourier(expr, k=None, x=None):
        expr = _to_expr(expr)
        k = _as_sym(k) if k is not None else _pick_var(expr, "k")
        x = _as_sym(x) if x is not None else sp.Symbol("x")
        try:
            return sp.inverse_fourier_transform(expr, k, x)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"傅里叶逆变换失败：{exc}")

    def mellin(expr, x=None, s=None):
        expr = _to_expr(expr)
        x = _as_sym(x) if x is not None else _pick_var(expr, "x")
        s = _as_sym(s) if s is not None else sp.Symbol("s")
        try:
            out = sp.mellin_transform(expr, x, s)
            return out[0] if isinstance(out, tuple) else out
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"梅林变换失败：{exc}")

    def separate_variables(expr, *symbols):
        expr = _to_expr(expr)
        try:
            return sp.separatevars(expr, symbols=list(symbols) or None, force=True)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"分离变量失败：{exc}")

    def greens_function(x=None, xi=None, L=1):
        # 标准 1D 算子 -d²/dx² 在 [0, L] 上、Dirichlet 边界 u(0)=u(L)=0 的格林函数。
        x = _as_sym(x) if x is not None else sp.Symbol("x")
        xi = _as_sym(xi) if xi is not None else sp.Symbol("xi")
        L = sp.sympify(L)
        return sp.Piecewise(
            (x * (L - xi) / L, x <= xi),
            (xi * (L - x) / L, True),
        )

    return {
        "laplace": laplace, "inverse_laplace": inverse_laplace,
        "fourier": fourier, "inverse_fourier": inverse_fourier, "mellin": mellin,
        "separate_variables": separate_variables, "greens_function": greens_function,
        # 特殊函数：直接暴露 SymPy 函数对象，调用即得未求值/已求值表达式（自动 LaTeX）。
        "besselj": sp.besselj, "bessely": sp.bessely,
        "legendre": sp.legendre, "hermite": sp.hermite, "laguerre": sp.laguerre,
        "chebyshevt": sp.chebyshevt, "chebyshevu": sp.chebyshevu, "Ynm": sp.Ynm,
    }


# ===========================================================================
# 命令库 —— 阶段 3：张量分析 + 微分几何（轻量、可扩展：加曲率量 = 加一个函数）
# 约定：度规 g 为 n×n sp.Matrix，coords 为坐标符号列表 [r, theta, ...]。
# ===========================================================================
def _christoffel_array(g, X):
    """返回完整 Christoffel 数组 Gamma[k][i][j]（含零分量，供曲率张量内部求和）。"""
    n = len(X)
    ginv = g.inv()
    Gamma = [[[sp.Integer(0)] * n for _ in range(n)] for _ in range(n)]
    for k in range(n):
        for i in range(n):
            for j in range(n):
                s = sp.Integer(0)
                for l in range(n):
                    s += ginv[k, l] * (sp.diff(g[j, l], X[i])
                                       + sp.diff(g[i, l], X[j])
                                       - sp.diff(g[i, j], X[l]))
                Gamma[k][i][j] = sp.simplify(s / 2)
    return Gamma


def _commands_geometry(ctx):
    def metric(rows, coords=None):  # noqa: ARG001 - coords 仅为书写习惯，实际逐命令再传
        return _as_matrix(rows)

    def christoffel(g, coords):
        g = _as_matrix(g)
        X = _coords_list(coords)
        Gamma = _christoffel_array(g, X)
        comps = {(k, i, j): Gamma[k][i][j]
                 for k in range(len(X)) for i in range(len(X)) for j in range(len(X))
                 if Gamma[k][i][j] != 0}
        fmt = lambda idx, nm: f"\\Gamma^{{{nm[idx[0]]}}}_{{{nm[idx[1]]}{nm[idx[2]]}}}"  # noqa: E731
        return TensorResult("Christoffel 符号 Γ^k_{ij}", comps, X, fmt=fmt)

    def _riemann_array(g, X):
        n = len(X)
        G = _christoffel_array(g, X)
        # R^l_{ijk} = ∂_i Γ^l_{jk} - ∂_j Γ^l_{ik} + Γ^l_{im}Γ^m_{jk} - Γ^l_{jm}Γ^m_{ik}
        R = [[[[sp.Integer(0)] * n for _ in range(n)] for _ in range(n)] for _ in range(n)]
        for l in range(n):
            for i in range(n):
                for j in range(n):
                    for k in range(n):
                        term = sp.diff(G[l][j][k], X[i]) - sp.diff(G[l][i][k], X[j])
                        for m in range(n):
                            term += G[l][i][m] * G[m][j][k] - G[l][j][m] * G[m][i][k]
                        R[l][i][j][k] = sp.simplify(term)
        return R

    def riemann(g, coords):
        g = _as_matrix(g)
        X = _coords_list(coords)
        R = _riemann_array(g, X)
        n = len(X)
        comps = {(l, i, j, k): R[l][i][j][k]
                 for l in range(n) for i in range(n) for j in range(n) for k in range(n)
                 if R[l][i][j][k] != 0}
        fmt = lambda idx, nm: f"R^{{{nm[idx[0]]}}}_{{{nm[idx[1]]}{nm[idx[2]]}{nm[idx[3]]}}}"  # noqa: E731
        return TensorResult("Riemann 曲率张量 R^l_{ijk}", comps, X, fmt=fmt)

    def ricci(g, coords):
        g = _as_matrix(g)
        X = _coords_list(coords)
        R = _riemann_array(g, X)
        n = len(X)
        # Ricci 缩并：R_{jk} = R^i_{ijk}（上指标与第一个下指标缩并，标准符号约定，
        # 使 2D 曲面满足 R = 2K，球面 R = +2/a²）。
        Ric = sp.zeros(n, n)
        for j in range(n):
            for k in range(n):
                Ric[j, k] = sp.simplify(sum(R[i][i][j][k] for i in range(n)))
        return Ric

    def ricci_scalar(g, coords):
        g = _as_matrix(g)
        X = _coords_list(coords)
        Ric = ricci(g, X)
        ginv = g.inv()
        n = len(X)
        return sp.simplify(sum(ginv[j, k] * Ric[j, k] for j in range(n) for k in range(n)))

    # —— 曲面局部几何（参数曲面 r(u, v)）—— #
    def _surface_forms(r, params):
        r = _as_vec(r)
        if not isinstance(params, (list, tuple)) or len(params) < 2:
            raise GuaxiMathError("请提供两个参数，例如 (u, v)。")
        u, v = _as_sym(params[0]), _as_sym(params[1])
        ru, rv = r.diff(u), r.diff(v)
        E, F, G = sp.simplify(ru.dot(ru)), sp.simplify(ru.dot(rv)), sp.simplify(rv.dot(rv))
        n = ru.cross(rv)
        nrm = sp.sqrt(n.dot(n))
        unit = n / nrm
        ruu, ruv, rvv = r.diff(u, 2), ru.diff(v), r.diff(v, 2)
        L, M, N = sp.simplify(ruu.dot(unit)), sp.simplify(ruv.dot(unit)), sp.simplify(rvv.dot(unit))
        return E, F, G, L, M, N

    def first_fundamental_form(r, params):
        E, F, G, _L, _M, _N = _surface_forms(r, params)
        return DecompResult("第一基本形式 (E, F, G)", [("E =", E), ("F =", F), ("G =", G)])

    def second_fundamental_form(r, params):
        _E, _F, _G, L, M, N = _surface_forms(r, params)
        return DecompResult("第二基本形式 (L, M, N)", [("L =", L), ("M =", M), ("N =", N)])

    def gaussian_curvature(r, params):
        E, F, G, L, M, N = _surface_forms(r, params)
        return sp.simplify((L * N - M ** 2) / (E * G - F ** 2))

    def mean_curvature(r, params):
        E, F, G, L, M, N = _surface_forms(r, params)
        return sp.simplify((E * N - 2 * F * M + G * L) / (2 * (E * G - F ** 2)))

    def geodesic_equations(g, coords, param="s"):
        g = _as_matrix(g)
        X = _coords_list(coords)
        s = _as_sym(param)
        G = _christoffel_array(g, X)
        n = len(X)
        funcs = [sp.Function(str(X[a]))(s) for a in range(n)]
        eqs = []
        for k in range(n):
            acc = sp.diff(funcs[k], s, 2)
            for i in range(n):
                for j in range(n):
                    gijk = G[k][i][j].subs({X[a]: funcs[a] for a in range(n)})
                    if gijk != 0:
                        acc += gijk * sp.diff(funcs[i], s) * sp.diff(funcs[j], s)
            eqs.append(sp.Eq(sp.simplify(acc), 0))
        return eqs

    return {
        "metric": metric, "christoffel": christoffel, "riemann": riemann,
        "ricci": ricci, "ricci_scalar": ricci_scalar,
        "first_fundamental_form": first_fundamental_form,
        "second_fundamental_form": second_fundamental_form,
        "gaussian_curvature": gaussian_curvature, "mean_curvature": mean_curvature,
        "geodesic_equations": geodesic_equations,
    }


# ===========================================================================
# 命令库 —— 阶段 4a：矩阵论（Jordan 标准形 / 矩阵函数 / Schur）
# ===========================================================================
def _commands_matrixtheory(ctx):
    def jordan(M):
        M = _as_matrix(M)
        try:
            P, J = M.jordan_form()
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"Jordan 标准形失败：{exc}")
        return DecompResult("Jordan 标准形 M = P J P⁻¹", [("P =", P), ("J =", J)])

    def matrix_exp(M):
        M = _as_matrix(M)
        try:
            return M.exp()
        except Exception:  # noqa: BLE001
            # 符号矩阵指数失败 → 退化为数值（scipy）。
            try:
                from scipy.linalg import expm
                A = np.array(M.evalf().tolist(), dtype=complex)
                return sp.Matrix(np.round(expm(A), 6))
            except Exception as exc:  # noqa: BLE001
                raise GuaxiMathError(f"矩阵指数失败：{exc}")

    def schur(M):
        M = _as_matrix(M)
        try:
            from scipy.linalg import schur as _schur
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"Schur 分解需要 scipy（{exc}）。")
        try:
            A = np.array(M.evalf().tolist(), dtype=float)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"Schur 分解需要数值矩阵：{exc}")
        T, Z = _schur(A)
        return DecompResult("Schur 分解（数值）M = Z T Zᵀ", [
            ("T =", sp.Matrix(np.round(T, 6))),
            ("Z =", sp.Matrix(np.round(Z, 6))),
        ])

    def is_diagonalizable(M):
        M = _as_matrix(M)
        return sp.sympify(bool(M.is_diagonalizable()))

    def cond(M):
        M = _as_matrix(M)
        try:
            A = np.array(M.evalf().tolist(), dtype=float)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"条件数需要数值矩阵：{exc}")
        return sp.Float(float(np.linalg.cond(A)))

    return {
        "jordan": jordan, "matrix_exp": matrix_exp, "schur": schur,
        "is_diagonalizable": is_diagonalizable, "cond": cond,
    }


# ===========================================================================
# 命令库 —— 阶段 4b：泛函分析（内积空间 / 正交化 / 算子谱 / L² 逼近）
# ===========================================================================
def _commands_functional(ctx):
    def inner_product(u, v, var=None, a=-1, b=1, weight=1):
        # 向量内积；或函数 L² 内积 ∫ u·conj(v)·weight dx。
        if isinstance(u, (sp.MatrixBase, list, tuple)) and isinstance(v, (sp.MatrixBase, list, tuple)):
            uu, vv = _as_vec(u), _as_vec(v)
            return sp.simplify(uu.dot(vv.conjugate()))
        u, v, w = _to_expr(u), _to_expr(v), _to_expr(weight)
        x = _as_sym(var) if var is not None else _pick_var(u * v, "x")
        return sp.integrate(u * sp.conjugate(v) * w, (x, sp.sympify(a), sp.sympify(b)))

    def gram_schmidt(vectors, orthonormal=False):
        if not isinstance(vectors, (list, tuple)) or not vectors:
            raise GuaxiMathError("请提供向量列表，例如 gram_schmidt([matrix([1,1,0]), matrix([1,0,1])])。")
        try:
            out = sp.GramSchmidt([_as_vec(v) for v in vectors], bool(orthonormal))
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"正交化失败：{exc}")
        return DecompResult("Gram–Schmidt 正交化", [("", v) for v in out])

    def norm(v, var=None, a=-1, b=1, weight=1):
        if isinstance(v, (sp.MatrixBase, list, tuple)):
            return sp.simplify(_as_vec(v).norm())
        v, w = _to_expr(v), _to_expr(weight)
        x = _as_sym(var) if var is not None else _pick_var(v, "x")
        return sp.sqrt(sp.integrate(v * sp.conjugate(v) * w, (x, sp.sympify(a), sp.sympify(b))))

    def operator_spectrum(M):
        M = _as_matrix(M)
        eigs = M.eigenvals()
        vals = sp.Matrix(list(eigs.keys()))
        return DecompResult("算子谱（特征值 / 谱半径）", [
            ("特征值 =", vals),
            ("谱半径 =", sp.Max(*[sp.Abs(k) for k in eigs.keys()])),
        ])

    def project(u, v):
        uu, vv = _as_vec(u), _as_vec(v)
        coeff = uu.dot(vv.conjugate()) / vv.dot(vv.conjugate())
        return sp.simplify(coeff) * vv

    def l2_approx(f, basis="legendre", var=None, a=-1, b=1, n=5):
        f = _to_expr(f)
        x = _as_sym(var) if var is not None else _pick_var(f, "x")
        n = int(n)
        if str(basis).lower() == "legendre":
            phis = [sp.legendre(k, x) for k in range(n + 1)]
        elif str(basis).lower() == "fourier":
            phis = [sp.Integer(1)] + [g for k in range(1, n + 1)
                                      for g in (sp.cos(k * x), sp.sin(k * x))]
        else:
            raise GuaxiMathError("basis 仅支持 'legendre' 或 'fourier'。")
        approx = sp.Integer(0)
        for phi in phis:
            num = sp.integrate(f * phi, (x, sp.sympify(a), sp.sympify(b)))
            den = sp.integrate(phi * phi, (x, sp.sympify(a), sp.sympify(b)))
            if den != 0:
                approx += sp.nsimplify(num / den) * phi
        return sp.simplify(approx)

    return {
        "inner_product": inner_product, "gram_schmidt": gram_schmidt, "norm": norm,
        "operator_spectrum": operator_spectrum, "project": project, "l2_approx": l2_approx,
    }


# ===========================================================================
# 命令库 —— 阶段 B（样板）：数值计算（积分 / ODE / 求根 / 优化 / 最小二乘）
# 重型结果登记 carrier，可在右侧面板 / 独立工作台里复跑、调精度、看收敛过程。
# ===========================================================================
def _commands_numerical(ctx):
    def nintegrate(f, rng=None):
        f = _to_expr(f)
        if rng is None:
            raise GuaxiMathError("数值积分需要区间，例如 nintegrate(exp(-x**2), (x, -5, 5))。")
        sym, a, b = _coerce_range(rng)
        from scipy.integrate import quad
        fn = sp.lambdify(sym, f, "numpy")
        val, err = quad(lambda t: float(np.real(fn(t))), a, b)
        return NumericResult(sp.Float(val), method="quad", title="数值积分",
                             converged=True, note=f"绝对误差≈{err:.2e}")

    def nsolve(f, x0, tol=1e-10, maxiter=50):
        f = _to_expr(f)
        x = _pick_var(f, "x")
        fn = sp.lambdify(x, f, "numpy")
        fpn = sp.lambdify(x, sp.diff(f, x), "numpy")
        xi = float(x0)
        iters = []
        converged = False
        for k in range(int(maxiter)):
            fv = float(np.real(fn(xi)))
            iters.append({"n": k, "x": xi, "residual": abs(fv)})
            if abs(fv) < float(tol):
                converged = True
                break
            d = float(np.real(fpn(xi)))
            if d == 0:
                break
            xi = xi - fv / d
        key = ctx.add_carrier("numeric", "nsolve", label="数值求根",
                              params={"x0": float(x0), "tol": float(tol), "maxiter": int(maxiter),
                                      "expr": sp.sstr(f), "var": str(x)},
                              summary=f"root≈{xi:.6g}")
        return NumericResult(sp.Float(xi), iterations=iters, method="Newton",
                             tol=float(tol), converged=converged, title="数值求根",
                             note=f"工作台键 {key}" if key else "")

    def minimize(f, x0, method="BFGS", tol=1e-8, maxiter=200):
        from scipy.optimize import minimize as _min
        f = _to_expr(f)
        syms = sorted(_free_symbols(f), key=str)
        if not syms:
            raise GuaxiMathError("目标函数没有变量。")
        fn = sp.lambdify(syms, f, "numpy")
        x0v = np.atleast_1d(np.array(x0 if isinstance(x0, (list, tuple)) else [x0], dtype=float))

        def obj(v):
            return float(np.real(fn(*np.atleast_1d(v))))

        iters = []

        def cb(xk):
            iters.append({"n": len(iters), "x": [float(v) for v in np.atleast_1d(xk)],
                          "residual": obj(xk)})

        res = _min(obj, x0v, method=str(method), tol=float(tol),
                   callback=cb, options={"maxiter": int(maxiter)})
        value = sp.Float(float(res.x[0])) if res.x.size == 1 else sp.Matrix(np.round(res.x, 8))
        key = ctx.add_carrier("numeric", "minimize", label="数值优化",
                              params={"x0": [float(v) for v in x0v], "method": str(method),
                                      "tol": float(tol), "maxiter": int(maxiter),
                                      "expr": sp.sstr(f), "vars": [str(s) for s in syms]},
                              summary=f"f*≈{float(res.fun):.6g}")
        return NumericResult(value, iterations=iters, method=str(method), tol=float(tol),
                             converged=bool(res.success), title="数值优化",
                             note=(f"f*≈{float(res.fun):.6g}" + (f"，工作台键 {key}" if key else "")))

    def lstsq(A, b):
        try:
            Am = np.array(_as_matrix(A).evalf().tolist(), dtype=float)
            bm = np.array(_as_matrix(b).evalf().tolist(), dtype=float).reshape(-1)
        except Exception as exc:  # noqa: BLE001
            raise GuaxiMathError(f"最小二乘需要数值矩阵：{exc}")
        sol, res, rank, _sv = np.linalg.lstsq(Am, bm, rcond=None)
        note = f"秩 {int(rank)}" + (f"，残差平方和≈{float(res[0]):.4g}" if np.size(res) else "")
        return NumericResult(sp.Matrix(np.round(sol, 6)), method="lstsq",
                             title="最小二乘解", converged=True, note=note)

    def odeint(rhs, y0, trange, n=200, vars=None):
        from scipy.integrate import solve_ivp
        sym_t, a, b = _coerce_range(trange)
        is_system = isinstance(rhs, (list, tuple))
        if is_system:
            exprs = [_to_expr(r) for r in rhs]
            if vars:
                state = [_as_sym(v) for v in vars]
            else:
                fs = sorted(_free_symbols(exprs) - {sym_t}, key=str)
                state = fs[:len(exprs)]
            if len(state) != len(exprs):
                raise GuaxiMathError("状态变量个数与方程个数不一致，请用 vars=[...] 指定。")
            y0v = [float(v) for v in (y0 if isinstance(y0, (list, tuple)) else [y0])]
            fns = [sp.lambdify([sym_t] + state, ex, "numpy") for ex in exprs]

            def rhs_fn(t, Y):
                return [float(np.real(fn(t, *Y))) for fn in fns]
        else:
            ex = _to_expr(rhs)
            yv = _as_sym(vars[0]) if vars else sp.Symbol("y")
            state = [yv]
            y0v = [float(y0 if not isinstance(y0, (list, tuple)) else y0[0])]
            fn = sp.lambdify([sym_t, yv], ex, "numpy")

            def rhs_fn(t, Y):
                return [float(np.real(fn(t, Y[0])))]

        ts = np.linspace(a, b, int(n))
        sol = solve_ivp(rhs_fn, (a, b), y0v, t_eval=ts, dense_output=False)
        if not sol.success:
            raise GuaxiMathError(f"ODE 数值求解失败：{sol.message}")
        # 把解数组 stash 成会话变量，供工作台逐帧回放（轻量仿真扩展缝）。
        sol_var = ""
        if ctx.session is not None:
            seq = int(ctx.session.get("__GUAXIMATH_CARRIER_SEQ__", 0)) + 1
            sol_var = f"__gx_ode_sol_{seq}"
            ctx.session[sol_var] = {"t": sol.t.tolist(), "y": sol.y.tolist(),
                                    "state": [str(s) for s in state]}
        key = ctx.add_carrier("simulation", "odeint", label="ODE 数值解",
                              params={"rhs": [sp.sstr(_to_expr(r)) for r in (rhs if is_system else [rhs])],
                                      "y0": y0v, "trange": [str(sym_t), a, b], "n": int(n),
                                      "state": [str(s) for s in state]},
                              summary=f"{len(state)} 维，t∈[{a:g},{b:g}]",
                              solution_var=sol_var)
        # 内联轨迹图：各分量随 t。
        fig = None
        if _HAS_PLOTLY:
            fig = go.Figure()
            for i, s in enumerate(state):
                fig.add_trace(go.Scatter(x=sol.t, y=sol.y[i], mode="lines", name=str(s)))
            fig.update_layout(title="ODE 数值解（分量 vs t）", height=420,
                              xaxis_title=str(sym_t), yaxis_title="state", hovermode="x unified")
            ctx.blocks.append({"kind": "heading", "text": "ODE 数值解"})
            ctx.text(f"（{len(state)} 维系统，t∈[{a:g}, {b:g}]，{int(n)} 个采样点"
                     + (f"，工作台键 {key}" if key else "") + "）")
            ctx.blocks.append({"kind": "figure", "fig": fig, "png": None, "caption": ""})
            return None  # 已直接写入结果块
        return NumericResult(sp.Float(float(sol.y[0][-1])), method="solve_ivp",
                             converged=True, title="ODE 数值解",
                             note=f"末值≈{float(sol.y[0][-1]):.6g}")

    return {
        "nintegrate": nintegrate, "nsolve": nsolve, "minimize": minimize,
        "lstsq": lstsq, "odeint": odeint,
    }


# ===========================================================================
# 命令库 —— 阶段 B（样板）：仿真（轻量、可扩展：相图 / 轨迹 + Simulator 扩展缝）
# ===========================================================================
def _commands_sim(ctx):
    def phase_portrait(rhs, xr, yr, density=18, vars=None):
        if not isinstance(rhs, (list, tuple)) or len(rhs) != 2:
            raise GuaxiMathError("相图需要二维系统，例如 phase_portrait([y, -x], (x,-3,3), (y,-3,3))。")
        exprs = [_to_expr(r) for r in rhs]
        if vars:
            state = [_as_sym(v) for v in vars]
        else:
            fs = sorted(_free_symbols(exprs), key=str)
            state = fs[:2]
        if len(state) != 2:
            raise GuaxiMathError("无法推断两个状态变量，请用 vars=[x, y] 指定。")
        xsym, ysym = state
        (_sx, x0, x1) = _coerce_range(xr if _looks_like_range(xr) else (xsym, xr[0], xr[1]))
        (_sy, y0, y1) = _coerce_range(yr if _looks_like_range(yr) else (ysym, yr[0], yr[1]))
        d = max(5, int(density))
        xs = np.linspace(x0, x1, d)
        ys = np.linspace(y0, y1, d)
        X, Y = np.meshgrid(xs, ys)
        fu = sp.lambdify([xsym, ysym], exprs[0], "numpy")
        fv = sp.lambdify([xsym, ysym], exprs[1], "numpy")
        U = np.real(fu(X, Y) + 0 * X)
        V = np.real(fv(X, Y) + 0 * Y)
        key = ctx.add_carrier("simulation", "phase_portrait", label="相图 / 向量场",
                              params={"rhs": [sp.sstr(e) for e in exprs],
                                      "xr": [str(xsym), x0, x1], "yr": [str(ysym), y0, y1],
                                      "density": d, "state": [str(s) for s in state]},
                              summary="二维向量场")
        if not _HAS_PLOTLY:
            raise GuaxiMathError("绘制相图需要 plotly，请先安装。")
        import plotly.figure_factory as ff
        mag = np.hypot(U, V)
        scale = 0.3 / (np.nanmax(mag) or 1.0)
        fig = ff.create_quiver(X, Y, U, V, scale=scale, arrow_scale=0.4,
                               line=dict(width=1), name="向量场")
        fig.update_layout(title="相图（向量场）", height=480,
                          xaxis_title=str(xsym), yaxis_title=str(ysym))
        ctx.text("相图 / 向量场" + (f"（工作台键 {key}）" if key else ""))
        ctx.blocks.append({"kind": "figure", "fig": fig, "png": None, "caption": ""})
        return None

    return {"phase_portrait": phase_portrait}


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
    # 阶段 2a 复变函数
    "residue": "留数", "laurent": "洛朗展开", "contour_integral": "围道积分",
    "conformal": "共形映射 f'(z)", "cauchy_riemann": "柯西–黎曼方程",
    # 阶段 2b 数学物理方法
    "laplace": "拉普拉斯变换", "inverse_laplace": "拉普拉斯逆变换",
    "fourier": "傅里叶变换", "inverse_fourier": "傅里叶逆变换", "mellin": "梅林变换",
    "separate_variables": "分离变量", "greens_function": "格林函数",
    "besselj": "贝塞尔函数", "legendre": "勒让德多项式", "hermite": "厄米多项式",
    "laguerre": "拉盖尔多项式", "chebyshevt": "切比雪夫多项式",
    # 阶段 3 张量 / 微分几何
    "ricci": "Ricci 张量", "ricci_scalar": "标量曲率",
    "gaussian_curvature": "高斯曲率", "mean_curvature": "平均曲率",
    "geodesic_equations": "测地线方程",
    # 阶段 4a 矩阵论
    "matrix_exp": "矩阵指数", "is_diagonalizable": "可对角化", "cond": "条件数",
    # 阶段 4b 泛函分析
    "inner_product": "内积", "norm": "范数", "project": "投影",
    "l2_approx": "L² 最佳逼近",
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
    ctx.session = ns
    ctx.carriers = ns.setdefault("__GUAXIMATH_CARRIERS__", {})
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
