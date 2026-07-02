#!/usr/bin/env bash
# 构建 MinerU 强引擎运行环境（独立 venv，与 marker 隔离）。
#
# 产物布局（与 engine.rs::resolve_mineru_* 对齐）：
#   src-tauri/utils/MinerU/.venv                    # 独立 Python venv
#   src-tauri/utils/MinerU/guaxi_mineru_convert.py  # 包装脚本
#   src-tauri/utils/Models/mineru_models            # 模型缓存目录
#
# MinerU 体积大、依赖 GPU（可选），因此作为可选组件：未构建时应用自动回退到 marker。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

MINERU_SRC="$ROOT_DIR/Convertor/MinerU"
MINERU_PARENT="$ROOT_DIR/src-tauri/utils"
MINERU_DST="$MINERU_PARENT/MinerU"

VENV_DIR="$MINERU_DST/.venv"
MODELS_DIR="$ROOT_DIR/src-tauri/utils/Models/mineru_models"

PYTHON_VERSION="${PYTHON_VERSION:-3.10}"
CONDA_OVERRIDE_CHANNELS="${CONDA_OVERRIDE_CHANNELS:-1}"
CONDA_CHANNEL="${CONDA_CHANNEL:-conda-forge}"
# MinerU 发行包名，可用 MINERU_PIP 覆盖（如需固定版本 mineru==2.x 或改用 magic-pdf）。
MINERU_PIP="${MINERU_PIP:-mineru[core]}"

mkdir -p "$MINERU_DST"
mkdir -p "$MODELS_DIR"

# 拷贝包装脚本到目标目录
if [ -f "$MINERU_SRC/guaxi_mineru_convert.py" ]; then
  cp -f "$MINERU_SRC/guaxi_mineru_convert.py" "$MINERU_DST/guaxi_mineru_convert.py"
else
  echo "找不到 guaxi_mineru_convert.py（应在 $MINERU_SRC/）" >&2
  exit 1
fi

to_windows_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
    return 0
  fi
  printf "%s" "$1"
}

CONDA_BIN="${CONDA_BIN:-}"
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

"$VENV_PY" -m pip install -U pip setuptools wheel

# 安装 MinerU。默认走 CPU/自动检测；如需 CUDA 轮子请自行设置 MINERU_PIP 或额外安装 torch。
echo "Installing MinerU: $MINERU_PIP"
"$VENV_PY" -m pip install "$MINERU_PIP" || {
  echo "mineru 安装失败，尝试回退安装 magic-pdf（MinerU 1.x）..." >&2
  "$VENV_PY" -m pip install "magic-pdf[full]" || {
    echo "MinerU 安装失败。应用将在运行时自动回退到 marker。" >&2
    exit 1
  }
}

echo
echo "MinerU env ready:"
echo "  MinerU dir:  $MINERU_DST"
echo "  Python:      $VENV_PY"
echo "  Wrapper:     $MINERU_DST/guaxi_mineru_convert.py"
echo "  Models:      $MODELS_DIR"
echo
echo "提示：模型首次运行会自动下载到 $MODELS_DIR（受 HF_HOME 等环境变量控制）。"
