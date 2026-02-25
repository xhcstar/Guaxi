set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LIB_DIR="$ROOT_DIR/src-tauri/utils/python_libs"

PYTHON_EXE="${PYTHON_EXE:-}"
if [ -z "$PYTHON_EXE" ]; then
  CANDIDATE="$ROOT_DIR/src-tauri/utils/MarkdownConvertor/marker-1.10.1/.venv/Scripts/python.exe"
  if [ -f "$CANDIDATE" ]; then
    PYTHON_EXE="$CANDIDATE"
  else
    PYTHON_EXE="python"
  fi
fi

echo "python=$PYTHON_EXE"
echo "lib_dir=$LIB_DIR"

mkdir -p "$LIB_DIR"

TO_WIN_PATH=""
if command -v cygpath >/dev/null 2>&1; then
  TO_WIN_PATH="$(cygpath -w "$LIB_DIR")"
fi

TARGET_DIR="$LIB_DIR"
if [ -n "$TO_WIN_PATH" ]; then
  TARGET_DIR="$TO_WIN_PATH"
fi

echo "target_dir=$TARGET_DIR"

"$PYTHON_EXE" -m pip install -U modelscope -t "$TARGET_DIR"

if [ ! -d "$LIB_DIR" ] || [ -z "$(ls -A "$LIB_DIR" 2>/dev/null)" ]; then
  echo "python_libs is empty after install" >&2
  exit 1
fi

echo "done"