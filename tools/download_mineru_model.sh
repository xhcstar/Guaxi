#!/usr/bin/env bash
# 预下载 MinerU 模型到 src-tauri/utils/Models/mineru_models。
#
# 说明：
#   - 使用 MinerU 独立 venv 的 python（由 build_mineru_env.sh 构建）。
#   - 通过 HF_HOME / MODELSCOPE_CACHE 等环境变量把模型缓存指向项目目录，
#     与运行时 engine.rs 设置的 mineru_models 目录一致。
#   - 未安装 MinerU 时直接跳过（应用会自动回退 marker）。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MINERU_DIR="$REPO_ROOT/src-tauri/utils/MinerU"
VENV_DIR="$MINERU_DIR/.venv"
MODELS_DIR="$REPO_ROOT/src-tauri/utils/Models/mineru_models"
# 模型来源：huggingface（默认）或 modelscope（国内更快）。
MINERU_MODEL_SOURCE="${MINERU_MODEL_SOURCE:-modelscope}"

mkdir -p "$MODELS_DIR"

find_python() {
  for p in "$VENV_DIR/bin/python" "$VENV_DIR/Scripts/python.exe" "$VENV_DIR/python.exe"; do
    if [ -x "$p" ] || [ -f "$p" ]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

PY="$(find_python || true)"
if [ -z "${PY:-}" ]; then
  echo "[MinerU] 未找到 MinerU venv，请先运行 build_mineru_env.sh。跳过模型下载。" >&2
  exit 0
fi

export HF_HOME="$MODELS_DIR"
export HUGGINGFACE_HUB_CACHE="$MODELS_DIR"
export MODELSCOPE_CACHE="$MODELS_DIR"
export TRANSFORMERS_CACHE="$MODELS_DIR"
export TORCH_HOME="$MODELS_DIR"
export XDG_CACHE_HOME="$MODELS_DIR"
export MINERU_MODEL_SOURCE

echo "[MinerU] 模型缓存目录: $MODELS_DIR"
echo "[MinerU] 模型来源: $MINERU_MODEL_SOURCE"

# MinerU 2.x 提供 CLI 下载器；不同版本入口不同，逐一尝试。
if "$PY" -c "import mineru" 2>/dev/null; then
  echo "[MinerU] 使用 mineru 内置下载器"
  "$PY" -m mineru.cli.models_download 2>/dev/null \
    || "$PY" -m mineru.model.download 2>/dev/null \
    || echo "[MinerU] 内置下载器不可用，模型将在首次转换时按需自动下载。"
elif "$PY" -c "import magic_pdf" 2>/dev/null; then
  echo "[MinerU] 检测到 magic_pdf(1.x)，模型将在首次运行时按需下载。"
else
  echo "[MinerU] 未检测到 mineru/magic_pdf，跳过。" >&2
fi

echo "[MinerU] 完成（若未预下载，首次转换会自动补全模型）。"
