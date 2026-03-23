#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CLEAN_ONLY="${1:-}"

if command -v powershell.exe >/dev/null 2>&1; then
  POWERSHELL_BIN="powershell.exe"
elif command -v powershell >/dev/null 2>&1; then
  POWERSHELL_BIN="powershell"
else
  echo "powershell not found" >&2
  exit 1
fi

to_windows_path() {
  local input="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$input"
  else
    printf '%s\n' "$input"
  fi
}

compress_dir() {
  local source_dir="$1"
  local source_abs="$ROOT_DIR/$source_dir"
  local zip_abs="${source_abs}.zip"
  local source_win
  local zip_win

  if [ ! -d "$source_abs" ]; then
    echo "skip missing directory: $source_dir" >&2
    return 0
  fi

  source_win="$(to_windows_path "$source_abs")"
  zip_win="$(to_windows_path "$zip_abs")"

  if [ "$CLEAN_ONLY" = "--clean-only" ]; then
    "$POWERSHELL_BIN" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \
      "\$target = '$zip_win'; if (Test-Path -LiteralPath \$target) { Remove-Item -LiteralPath \$target -Force }; Write-Host \"Removed \$target\""
    return 0
  fi

  echo "compressing $source_dir -> ${source_dir}.zip"
  "$POWERSHELL_BIN" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \
    "\$src = '$source_win'; \$dst = '$zip_win'; if (Test-Path -LiteralPath \$dst) { Remove-Item -LiteralPath \$dst -Force }; Compress-Archive -Path (Join-Path \$src '*') -DestinationPath \$dst -CompressionLevel Optimal"
}

TARGETS=(
  "src-tauri/utils/SlidevEnv"
  "src-tauri/utils/sherpa-onnx"
  "src-tauri/utils/whisper"
  "src-tauri/utils/llama"
  "src-tauri/utils/python_libs"
  "src-tauri/utils/Models/tts"
  "src-tauri/utils/Models/whisper_models"
)

for target in "${TARGETS[@]}"; do
  compress_dir "$target"
done

if [ "$CLEAN_ONLY" = "--clean-only" ]; then
  echo "bundled resource archives cleaned"
else
  echo "bundled resource archives prepared"
fi
