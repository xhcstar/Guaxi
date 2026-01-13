#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

SURYA_SRC="$ROOT_DIR/Convertor/surya-0.17.0"
MARKER_SRC="$ROOT_DIR/Convertor/marker-1.10.1"

MARKER_PARENT="$ROOT_DIR/utils/MarkdownConvertor"
MARKER_DST="$MARKER_PARENT/marker-1.10.1"

VENV_DIR="$MARKER_DST/.venv"
MODELS_DIR="$ROOT_DIR/utils/Models/surya_models"

PYTHON_VERSION="${PYTHON_VERSION:-3.10}"
MINICONDA_DIR="${MINICONDA_DIR:-$ROOT_DIR/utils/Miniconda}"

mkdir -p "$MARKER_PARENT"
mkdir -p "$MODELS_DIR"

if [ ! -d "$MARKER_DST" ]; then
  cp -a "$MARKER_SRC" "$MARKER_PARENT/"
fi

CONDA_BIN=""
if [ -f "$MINICONDA_DIR/Scripts/conda.exe" ]; then
  CONDA_BIN="$MINICONDA_DIR/Scripts/conda.exe"
elif command -v conda >/dev/null 2>&1; then
  CONDA_BIN="conda"
fi

if [ -z "$CONDA_BIN" ]; then
  echo "conda not found. Please bundle Miniconda at: $MINICONDA_DIR" >&2
  exit 1
fi

if [ ! -d "$VENV_DIR" ]; then
  "$CONDA_BIN" create -y -p "$VENV_DIR" "python=$PYTHON_VERSION" pip
fi

if [ -f "$VENV_DIR/python.exe" ]; then
  VENV_PY="$VENV_DIR/python.exe"
else
  VENV_PY="$VENV_DIR/bin/python"
fi

"$VENV_PY" -m pip install -U pip setuptools wheel

"$VENV_PY" -m pip install "$MARKER_DST"

"$VENV_PY" -m pip install --force-reinstall --no-deps "$SURYA_SRC"

echo
echo "Marker env ready:"
echo "  Marker dir:  $MARKER_DST"
echo "  Python:      $VENV_PY"
echo "  Surya models:$MODELS_DIR"