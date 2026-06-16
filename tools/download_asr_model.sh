#!/usr/bin/env bash
# Download the sherpa-onnx SenseVoice ASR model into:
#   src-tauri/utils/Models/asr/sense-voice
#
# Design:
#   - Default to the existing ModelScope Python helper, matching
#     settingview.rs::run_modelscope_download.
#   - Use the Marker virtual-env Python without changing that environment.
#   - Use a per-target MODELSCOPE_CACHE directory, so stale user-cache locks
#     cannot block this project download.
#   - Download only the files required by runtime: model(.int8).onnx + tokens.txt.
#   - Keep GUAXI_ASR_SOURCE=github as an explicit archive fallback.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$REPO_ROOT/src-tauri/utils/Models/asr/sense-voice"
MODEL_NAME="sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"
DEFAULT_MODELSCOPE_MODEL_ID="csukuangfj/test-sense-voice-2026-04-03"
DEFAULT_MODELSCOPE_MODEL_FILE="model-5-seconds.onnx"
MODELSCOPE_MODEL_ID="${GUAXI_ASR_MODEL_ID:-$DEFAULT_MODELSCOPE_MODEL_ID}"
GITHUB_ARCHIVE="${GUAXI_ASR_MODEL_URL:-https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/${MODEL_NAME}.tar.bz2}"

file_size() {
  if [ -f "$1" ]; then
    wc -c < "$1" | tr -d '[:space:]'
  else
    echo 0
  fi
}

asr_model_ready() {
  local int8_path="$TARGET_DIR/model.int8.onnx"
  local full_path="$TARGET_DIR/model.onnx"
  local tokens_path="$TARGET_DIR/tokens.txt"
  local has_int8=0
  local has_full=0
  local has_tokens=0

  [ "$(file_size "$int8_path")" -ge 52428800 ] && has_int8=1
  [ "$(file_size "$full_path")" -ge 104857600 ] && has_full=1
  [ "$(file_size "$tokens_path")" -ge 1024 ] && has_tokens=1

  if { [ "$has_int8" -eq 1 ] || [ "$has_full" -eq 1 ]; } && [ "$has_tokens" -eq 1 ]; then
    echo "[ASR] Valid model already present: $TARGET_DIR (skip)"
    return 0
  fi

  echo "[ASR] Model validation status: int8=$has_int8 full=$has_full tokens=$has_tokens"
  return 1
}

find_modelscope_python() {
  if [ -n "${GUAXI_MODELSCOPE_PYTHON:-}" ]; then
    if [ -x "$GUAXI_MODELSCOPE_PYTHON" ] || [ -f "$GUAXI_MODELSCOPE_PYTHON" ]; then
      echo "$GUAXI_MODELSCOPE_PYTHON"
      return 0
    fi
    echo "GUAXI_MODELSCOPE_PYTHON does not exist: $GUAXI_MODELSCOPE_PYTHON" >&2
    return 1
  fi

  local venv_dir="$REPO_ROOT/src-tauri/utils/MarkdownConvertor/marker-1.10.1/.venv"
  local candidates=(
    "$venv_dir/bin/python"
    "$venv_dir/Scripts/python.exe"
    "$venv_dir/python.exe"
  )
  local p
  for p in "${candidates[@]}"; do
    if [ -x "$p" ] || [ -f "$p" ]; then
      echo "$p"
      return 0
    fi
  done

  echo "Cannot find Marker virtual-env python under: $venv_dir" >&2
  return 1
}

modelscope_download() {
  local source_filename="$1"
  local target_filename="$2"
  local min_bytes="$3"
  local python
  local script="$REPO_ROOT/src-tauri/scripts/modelscope_download.py"
  local downloaded_file="$TARGET_DIR/$source_filename"
  local out_file="$TARGET_DIR/$target_filename"
  local cache_root="$TARGET_DIR/_modelscope_cache"

  python="$(find_modelscope_python)"
  [ -f "$script" ] || { echo "modelscope helper script not found: $script" >&2; return 1; }

  mkdir -p "$TARGET_DIR"
  mkdir -p "$cache_root"
  rm -f "$out_file"
  if [ "$downloaded_file" != "$out_file" ]; then
    rm -f "$downloaded_file"
  fi

  echo "[ASR] ModelScope download start: model_id=$MODELSCOPE_MODEL_ID file=$source_filename target=$target_filename"
  echo "[ASR] Python: $python"
  echo "[ASR] MODELSCOPE_CACHE: $cache_root"
  MODELSCOPE_CACHE="$cache_root" "$python" "$script" --model-id "$MODELSCOPE_MODEL_ID" --filename "$source_filename" --local-dir "$TARGET_DIR"

  if [ "$downloaded_file" != "$out_file" ]; then
    [ -f "$downloaded_file" ] || { echo "Downloaded source file missing: $downloaded_file" >&2; return 1; }
    mv -f "$downloaded_file" "$out_file"
    echo "[ASR] Normalized model filename: $source_filename -> $target_filename"
  fi

  if [ "$(file_size "$out_file")" -lt "$min_bytes" ]; then
    echo "Downloaded file too small or missing: $out_file" >&2
    return 1
  fi

  echo "[ASR] ModelScope download ok: $out_file"
}

install_from_github_archive() {
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN

  echo "[ASR] Downloading archive: $GITHUB_ARCHIVE"
  if command -v curl >/dev/null 2>&1; then
    curl -L --fail --retry 3 --retry-delay 2 -o "$tmp/${MODEL_NAME}.tar.bz2" "$GITHUB_ARCHIVE"
  else
    wget -O "$tmp/${MODEL_NAME}.tar.bz2" "$GITHUB_ARCHIVE"
  fi

  echo "[ASR] Extracting archive..."
  tar -xjf "$tmp/${MODEL_NAME}.tar.bz2" -C "$tmp"

  mkdir -p "$(dirname "$TARGET_DIR")"
  rm -rf "$TARGET_DIR"
  mv "$tmp/${MODEL_NAME}" "$TARGET_DIR"
  echo "[ASR] GitHub archive install ok: $TARGET_DIR"
}

remove_modelscope_cache() {
  local cache_dir
  for cache_dir in "$TARGET_DIR/_modelscope_download" "$TARGET_DIR/_modelscope_cache"; do
    if [ -d "$cache_dir" ]; then
      rm -rf "$cache_dir"
      echo "[ASR] Removed ModelScope cache: $cache_dir"
    fi
  done
}

if asr_model_ready; then
  exit 0
fi

if [ "${GUAXI_ASR_SOURCE:-}" = "github" ]; then
  echo "[ASR] Source selected: GitHub archive"
  install_from_github_archive
else
  echo "[ASR] Source selected: ModelScope Python SDK"
  if [ -n "${GUAXI_ASR_MODEL_FILE:-}" ]; then
    model_source_file="$GUAXI_ASR_MODEL_FILE"
  elif [ "$MODELSCOPE_MODEL_ID" = "$DEFAULT_MODELSCOPE_MODEL_ID" ]; then
    model_source_file="$DEFAULT_MODELSCOPE_MODEL_FILE"
  elif [ "${GUAXI_ASR_FULL:-}" = "1" ]; then
    model_source_file="model.onnx"
  else
    model_source_file="model.int8.onnx"
  fi

  if [ "$model_source_file" = "$DEFAULT_MODELSCOPE_MODEL_FILE" ]; then
    model_target_file="model.onnx"
  else
    model_target_file="$model_source_file"
  fi

  if [ "$model_target_file" = "model.onnx" ]; then
    model_min_bytes=104857600
  else
    model_min_bytes=52428800
  fi

  modelscope_download "$model_source_file" "$model_target_file" "$model_min_bytes"
  modelscope_download "tokens.txt" "tokens.txt" 1024
  remove_modelscope_cache
fi

if ! asr_model_ready; then
  echo "Validation failed: missing valid model(.int8).onnx or tokens.txt in $TARGET_DIR" >&2
  exit 1
fi

echo "[ASR] Done: $TARGET_DIR"
