#!/usr/bin/env bash
# ⚠️ 旧流程（Windows + conda 专用，已被自动装配取代，保留仅为兜底/排查）：
# marker 的运行环境现在由 App 在**启动时自动装配**（uv 建独立 venv，在
# `utils/MarkdownConvertor/.venv`，源码取自 Guaxi fork 的 guaxi-mods 分支，
# 有 NVIDIA GPU 时自动装 CUDA 版 torch；模型启动时后台下载）。
# 本脚本产出的 **conda 环境**（`marker-1.10.1/.venv`）不再被优先使用，
# 只在自动装配失败时作为回退；设置页也已不再提供“运行环境离线加载”。
# 新流程无需执行本脚本，也不需要 conda。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

SURYA_SRC="$ROOT_DIR/Convertor/surya-0.17.0"
MARKER_SRC="$ROOT_DIR/Convertor/marker-1.10.1"

MARKER_PARENT="$ROOT_DIR/src-tauri/utils/MarkdownConvertor"
MARKER_DST="$MARKER_PARENT/marker-1.10.1"

VENV_DIR="$MARKER_DST/.venv"
MODELS_DIR="$ROOT_DIR/src-tauri/utils/Models/surya_models"

PYTHON_VERSION="${PYTHON_VERSION:-3.10}"
CONDA_OVERRIDE_CHANNELS="${CONDA_OVERRIDE_CHANNELS:-1}"
CONDA_CHANNEL="${CONDA_CHANNEL:-conda-forge}"

mkdir -p "$MARKER_PARENT"
mkdir -p "$MODELS_DIR"

if [ ! -d "$MARKER_DST" ]; then
  cp -a "$MARKER_SRC" "$MARKER_PARENT/"
fi

if [ ! -f "$MARKER_DST/pyproject.toml" ]; then
  rm -rf "$MARKER_DST"
  cp -a "$MARKER_SRC" "$MARKER_PARENT/"
fi

CONDA_BIN="${CONDA_BIN:-}"

to_windows_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
    return 0
  fi
  printf "%s" "$1"
}

if [ -z "$CONDA_BIN" ] && [ -n "${CONDA_EXE:-}" ] && [ -f "$CONDA_EXE" ]; then
  CONDA_BIN="$CONDA_EXE"
fi

if [ -z "$CONDA_BIN" ] && command -v conda >/dev/null 2>&1; then
  CONDA_BIN="$(command -v conda)"
fi

if [ -z "$CONDA_BIN" ] && command -v conda.exe >/dev/null 2>&1; then
  CONDA_BIN="$(command -v conda.exe)"
fi

if [ -z "$CONDA_BIN" ] && [ -n "${USERPROFILE:-}" ]; then
  for p in \
    "$USERPROFILE/Miniconda3/Scripts/conda.exe" \
    "$USERPROFILE/miniconda3/Scripts/conda.exe" \
    "$USERPROFILE/Anaconda3/Scripts/conda.exe" \
    "$USERPROFILE/anaconda3/Scripts/conda.exe" \
    "D:/Softwares/miniconda/Scripts/conda.exe"; do
    if [ -f "$p" ]; then
      CONDA_BIN="$p"
      break
    fi
  done
fi

if [ -z "$CONDA_BIN" ]; then
  echo "conda not found. Put conda/conda.exe on PATH, or set CONDA_BIN / CONDA_EXE." >&2
  exit 1
fi

if [ ! -d "$VENV_DIR" ]; then
  VENV_DIR_FOR_CONDA="$VENV_DIR"
  case "$CONDA_BIN" in
    *.exe) VENV_DIR_FOR_CONDA="$(to_windows_path "$VENV_DIR")" ;;
  esac

  if [ "$CONDA_OVERRIDE_CHANNELS" = "1" ]; then
    "$CONDA_BIN" create -y -p "$VENV_DIR_FOR_CONDA" --override-channels -c "$CONDA_CHANNEL" "python=$PYTHON_VERSION" pip
  else
    "$CONDA_BIN" create -y -p "$VENV_DIR_FOR_CONDA" "python=$PYTHON_VERSION" pip
  fi
fi

if [ -f "$VENV_DIR/Scripts/python.exe" ]; then
  VENV_PY="$VENV_DIR/Scripts/python.exe"
elif [ -f "$VENV_DIR/python.exe" ]; then
  VENV_PY="$VENV_DIR/python.exe"
else
  VENV_PY="$VENV_DIR/bin/python"
fi

MARKER_DST_FOR_PIP="$MARKER_DST"
SURYA_SRC_FOR_PIP="$SURYA_SRC"
case "$VENV_PY" in
  *.exe)
    MARKER_DST_FOR_PIP="$(to_windows_path "$MARKER_DST")"
    SURYA_SRC_FOR_PIP="$(to_windows_path "$SURYA_SRC")"
    ;;
esac

"$VENV_PY" -m pip install -U pip setuptools wheel

"$VENV_PY" -m pip install "$MARKER_DST_FOR_PIP"

"$VENV_PY" -m pip install fastapi uvicorn python-multipart

# 快速引擎依赖：pymupdf4llm(PDF->MD，含 PyMuPDF) + markitdown(Office/HTML/EPUB->MD)。
# 这些库同时被 guaxi_probe.py（探针）与 guaxi_fast_convert.py（快速引擎）使用。
"$VENV_PY" -m pip install "pymupdf4llm>=0.0.17" "markitdown[all]>=0.0.1a2" || \
  "$VENV_PY" -m pip install pymupdf4llm markitdown

"$VENV_PY" -m pip install --force-reinstall --no-deps "$SURYA_SRC_FOR_PIP"

echo
echo "Marker env ready:"
echo "  Marker dir:  $MARKER_DST"
echo "  Python:      $VENV_PY"
echo "  Surya models:$MODELS_DIR"
echo "  Fast engine: pymupdf4llm + markitdown (installed in marker venv)"