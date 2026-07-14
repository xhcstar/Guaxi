#!/usr/bin/env bash
# Linux/macOS 下打包 Guaxi Blender Bridge 扩展。
# 与 package_plugin.ps1 等价：ZIP 根目录直接包含 manifest 与入口，排除 __pycache__/*.pyc。
#
# 用法：
#   ./guaxi_blender_addon/package_plugin.sh [输出路径.zip]
#
# 依赖：bash、zip（若无 zip 则回退到 python3 的 zipfile）。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="${SCRIPT_DIR}/guaxi_blender_bridge"

REQUIRED_FILES=(
    "blender_manifest.toml"
    "__init__.py"
    "protocol.schema.json"
    "build_plan.schema.json"
    "platform_presets.json"
    "asset_resolver.py"
    "scene_director.py"
    "build_executor.py"
    "plan_validator.py"
    "compliance.py"
    "scene_builder.py"
    "headless_runner.py"
)

for f in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f "${SOURCE_DIR}/${f}" ]]; then
        echo "[GuaxiBlenderBridge] 缺少打包所需文件：${SOURCE_DIR}/${f}" >&2
        exit 1
    fi
done

OUTPUT_PATH="${1:-}"
if [[ -z "${OUTPUT_PATH}" ]]; then
    OUTPUT_PATH="$(cd "${SCRIPT_DIR}/.." && pwd)/GuaxiBlenderBridge-0.1.0.zip"
fi
case "${OUTPUT_PATH}" in
    *.zip) : ;;
    *) echo "[GuaxiBlenderBridge] 插件输出必须是 .zip 文件：${OUTPUT_PATH}" >&2; exit 1 ;;
esac

mkdir -p "$(dirname "${OUTPUT_PATH}")"
rm -f "${OUTPUT_PATH}"

echo "[GuaxiBlenderBridge] 开始打包 Blender 5.2 扩展"
echo "[GuaxiBlenderBridge] 源目录：${SOURCE_DIR}"
echo "[GuaxiBlenderBridge] 输出：${OUTPUT_PATH}"

# 暂存目录，保证 ZIP 根只含受控文件。
STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "${STAGE_DIR}"' EXIT
for f in "${REQUIRED_FILES[@]}"; do
    cp "${SOURCE_DIR}/${f}" "${STAGE_DIR}/${f}"
done

if command -v zip >/dev/null 2>&1; then
    (cd "${STAGE_DIR}" && zip -q -X -r "${OUTPUT_PATH}" .)
else
    # 回退：无 zip 命令时用 python3 打包（Blender 环境通常自带 python3）。
    python3 - "$STAGE_DIR" "$OUTPUT_PATH" <<'PY'
import os, sys, zipfile
stage, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
    for name in sorted(os.listdir(stage)):
        zf.write(os.path.join(stage, name), name)
PY
fi

SIZE="$(wc -c < "${OUTPUT_PATH}" | tr -d ' ')"
echo "[GuaxiBlenderBridge] 打包完成，大小：${SIZE} bytes"
echo "[GuaxiBlenderBridge] 已包含 ${#REQUIRED_FILES[@]} 个文件，已排除 __pycache__/*.pyc。"
echo "[GuaxiBlenderBridge] 在 Blender 5.2 中使用 Install from Disk 安装该 ZIP。"
