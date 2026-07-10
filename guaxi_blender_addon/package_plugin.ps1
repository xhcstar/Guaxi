param(
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"

# 设计说明：Blender 的 Install from Disk 要求清单和 __init__.py 位于 ZIP 根目录，
# 因此只压缩 guaxi_blender_bridge 的内容，不把外层开发目录一起放入压缩包。
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

Compress-Archive -Path (Join-Path $SourceDirectory "*") -DestinationPath $OutputPath -Force

$Archive = Get-Item -LiteralPath $OutputPath
Write-Host "[GuaxiBlenderBridge] 打包完成，大小：$($Archive.Length) bytes"
Write-Host "[GuaxiBlenderBridge] 在 Blender 5.2 中使用 Install from Disk 安装该 ZIP。"

