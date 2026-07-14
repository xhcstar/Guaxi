param(
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"

# 设计说明：Blender 的 Install from Disk 要求 blender_manifest.toml 和 __init__.py 位于
# ZIP 根目录。为避免把 __pycache__、旧 ZIP、编辑器临时文件打进包里（这是打包报错和插件
# 加载旧代码的常见原因），这里先把需要的文件“暂存”到临时目录，再压缩临时目录。

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceDirectory = Join-Path $ScriptRoot "guaxi_blender_bridge"
$ManifestPath = Join-Path $SourceDirectory "blender_manifest.toml"
$InitPath = Join-Path $SourceDirectory "__init__.py"

if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "缺少 Blender 扩展清单：$ManifestPath"
}
if (-not (Test-Path -LiteralPath $InitPath -PathType Leaf)) {
    throw "缺少 Blender 插件入口：$InitPath"
}

# 必须打进包的文件清单（缺一即报错，避免打出半成品）。
$RequiredFiles = @(
    "blender_manifest.toml",
    "__init__.py",
    "protocol.schema.json",
    "build_plan.schema.json",
    "platform_presets.json",
    "asset_resolver.py",
    "procedural_asset_builder.py",
    "scene_director.py",
    "build_executor.py",
    "plan_validator.py",
    "compliance.py",
    "scene_builder.py",
    "headless_runner.py"
)

foreach ($file in $RequiredFiles) {
    $full = Join-Path $SourceDirectory $file
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) {
        throw "缺少打包所需文件：$full"
    }
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path (Split-Path -Parent $ScriptRoot) "GuaxiBlenderBridge-0.1.0.zip"
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if ([System.IO.Path]::GetExtension($OutputPath) -ne ".zip") {
    throw "插件输出必须是 .zip 文件：$OutputPath"
}

$OutputDirectory = Split-Path -Parent $OutputPath
if (-not (Test-Path -LiteralPath $OutputDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
}

Write-Host "[GuaxiBlenderBridge] 开始打包 Blender 5.2 扩展"
Write-Host "[GuaxiBlenderBridge] 源目录：$SourceDirectory"
Write-Host "[GuaxiBlenderBridge] 输出：$OutputPath"

# 暂存目录：确保 ZIP 根目录只包含受控文件，且不含 __pycache__ / *.pyc / 旧 ZIP。
$StageRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("guaxi_addon_" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $StageRoot -Force | Out-Null
try {
    foreach ($file in $RequiredFiles) {
        Copy-Item -LiteralPath (Join-Path $SourceDirectory $file) -Destination (Join-Path $StageRoot $file) -Force
    }

    if (Test-Path -LiteralPath $OutputPath) {
        Remove-Item -LiteralPath $OutputPath -Force
    }

    # 压缩暂存目录的内容（而非目录本身），保证 manifest 位于 ZIP 根。
    Compress-Archive -Path (Join-Path $StageRoot "*") -DestinationPath $OutputPath -Force
}
finally {
    Remove-Item -LiteralPath $StageRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$Archive = Get-Item -LiteralPath $OutputPath
Write-Host "[GuaxiBlenderBridge] 打包完成，大小：$($Archive.Length) bytes"
Write-Host "[GuaxiBlenderBridge] 已包含 $($RequiredFiles.Count) 个文件，已排除 __pycache__/*.pyc。"
Write-Host "[GuaxiBlenderBridge] 在 Blender 5.2 中使用 Install from Disk 安装该 ZIP。"
