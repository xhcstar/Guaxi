#Requires -Version 5.1
<#
.SYNOPSIS
把 Windows 发布目录打包成可发布的 7z 分卷（每卷小于 GitHub 的 2 GiB 单文件上限）。

.DESCRIPTION
GitHub Release 对单个资产有 2 GiB 的硬上限，而带语音模型与 RSSHub 的完整
utils 分包通常有 2-3 GB，没法作为一个文件上传。这里按固定卷大小切成
guaxi-<tag>-windows-x64-full.7z.001 / .002 / …，用户把全部卷下到同一个文件夹，
用 7-Zip 解压 .001 即可。

脚本同时负责：
  * 校验发布目录完整——缺 utils_bundles/ 或 extract_utils_bundles.exe，
    用户安装到一半就会中止；
  * 生成 SHA256SUMS.txt；
  * 生成 release-notes.md（安装说明，直接作为 Release 正文）；
  * 兜底检查每个产物都小于 2 GiB。

.PARAMETER BundleDir
npm run tauri -- build 产出的发布目录，默认 src-tauri/target/release/bundle/nsis。

.PARAMETER OutputDir
分卷输出目录，默认 release-assets（Release 只上传这里的文件）。

.PARAMETER StagingDir
暂存目录，默认 release-payload（打包完成后可以删掉）。

.PARAMETER VolumeSizeMb
单卷大小（MiB），默认 1800。不要超过 2048，要给 2 GiB 上限留余量。

.PARAMETER MinimumBundleMb
utils_bundles 的完整性下限（MiB），默认 500。低于这个值基本可以断定模型没有进包
（例如误设了 SHERPA_SKIP_MODELS）。确需发布精简包时用 -AllowSmallPayload 放行。

.EXAMPLE
pwsh ./tools/package_release.ps1

.EXAMPLE
pwsh ./tools/package_release.ps1 -Tag v0.1.1 -VolumeSizeMb 1500
#>
param(
    [string]$BundleDir = 'src-tauri/target/release/bundle/nsis',
    [string]$OutputDir = 'release-assets',
    [string]$StagingDir = 'release-payload',
    [string]$NotesPath = 'release-notes.md',
    [string]$Tag = '',
    [int]$VolumeSizeMb = 1800,
    [int]$MinimumBundleMb = 500,
    [switch]$AllowSmallPayload
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Resolve-RepoPath([string]$Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $RepoRoot $Path))
}

function Format-Size([long]$Bytes) {
    if ($Bytes -ge 1GB) { return ('{0:N2} GB' -f ($Bytes / 1GB)) }
    if ($Bytes -ge 1MB) { return ('{0:N0} MB' -f ($Bytes / 1MB)) }
    return ('{0:N0} KB' -f ($Bytes / 1KB))
}

function Write-Utf8File([string]$Path, [string]$Content, [switch]$Bom) {
    # 记事本打开 .txt 靠 BOM 判断编码；Markdown 正文则不能带 BOM，
    # 否则 GitHub 会把开头的 "##" 解析成普通文本。
    $encoding = New-Object System.Text.UTF8Encoding($Bom.IsPresent)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

# ---------------------------------------------------------------------------
# 0. 基本信息
# ---------------------------------------------------------------------------
$bundleRoot = Resolve-RepoPath $BundleDir
$outputRoot = Resolve-RepoPath $OutputDir
$stagingRoot = Resolve-RepoPath $StagingDir
$notesFile = Resolve-RepoPath $NotesPath

if (-not (Test-Path -LiteralPath $bundleRoot)) {
    throw "发布目录不存在：$bundleRoot`n先跑 npm run tauri -- build 生成安装器与外置分包。"
}

$tauriConfig = Get-Content -LiteralPath (Resolve-RepoPath 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$version = [string]$tauriConfig.version
if (-not $Tag) {
    $Tag = "v$version"
}
elseif ($Tag -ne "v$version") {
    # 安装器文件名里的版本来自 tauri.conf.json，分卷名来自 tag。
    # 两者不一致说明忘了升版本号，发出去的包会对不上号。
    throw "tag（$Tag）与 tauri.conf.json 的版本（$version）不一致：请先把 package.json / Cargo.toml / tauri.conf.json 的版本升到同一版再打 tag。"
}
$folderName = "guaxi-$Tag-windows-x64"

Write-Host "[package] 版本 $version · 发布目录 $bundleRoot"

# ---------------------------------------------------------------------------
# 1. 暂存：把发布目录整个复制成「用户解压后看到的目录」
# ---------------------------------------------------------------------------
if (Test-Path -LiteralPath $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
$stageDir = Join-Path $stagingRoot $folderName
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
Copy-Item -Path (Join-Path $bundleRoot '*') -Destination $stageDir -Recurse -Force

# ---------------------------------------------------------------------------
# 2. 完整性校验：这几样少任何一个，安装到一半就会弹框中止
# ---------------------------------------------------------------------------
$setupExe = Get-ChildItem -LiteralPath $stageDir -Filter '*_x64-setup.exe' -File | Select-Object -First 1
if (-not $setupExe) {
    throw "发布目录里没有 *_x64-setup.exe：$stageDir"
}

$helperExe = Join-Path $stageDir 'extract_utils_bundles.exe'
if (-not (Test-Path -LiteralPath $helperExe)) {
    throw "发布目录缺少 extract_utils_bundles.exe：$stageDir`n没有它安装器无法解包 utils，安装会直接失败。"
}

$bundlesDir = Join-Path $stageDir 'utils_bundles'
$manifestFile = Join-Path $bundlesDir 'bundle-manifest.json'
if (-not (Test-Path -LiteralPath $manifestFile)) {
    throw "发布目录缺少 utils_bundles/bundle-manifest.json：$bundlesDir"
}

$bundleParts = @(Get-ChildItem -LiteralPath $bundlesDir -Filter '*.tar.zst' -File)
if ($bundleParts.Count -eq 0) {
    throw "utils_bundles 里没有任何 *.tar.zst 分包：$bundlesDir"
}
$bundleBytes = [long](($bundleParts | Measure-Object -Property Length -Sum).Sum)
$bundleMb = [math]::Round($bundleBytes / 1MB)
Write-Host "[package] utils_bundles：$($bundleParts.Count) 个分包 · $(Format-Size $bundleBytes)"

if ($bundleMb -lt $MinimumBundleMb -and -not $AllowSmallPayload) {
    $message = "utils_bundles 只有 $bundleMb MB（下限 $MinimumBundleMb MB），基本可以断定语音模型或 RSSHub 没进包。`n" +
        '请确认构建时没有设置 SHERPA_SKIP_MODELS / RSSHUB_SKIP；确需发布精简包请加 -AllowSmallPayload。'
    throw $message
}

# ---------------------------------------------------------------------------
# 3. 目录内的安装说明（用户解压后第一眼就能看到）
# ---------------------------------------------------------------------------
$installNotes = @"
Guaxi $Tag · Windows x64 完整离线包
==================================================

本目录必须整体使用，不要只把 setup.exe 单独拷走
--------------------------------------------------
  $($setupExe.Name)      NSIS 安装器
  extract_utils_bundles.exe     安装期解包工具（安装器会自动调用）
  utils_bundles\                运行时资源分包（$($bundleParts.Count) 个 tar.zst + 清单）
  README_utils_bundles.txt      分包清单（核对用）

安装步骤
--------------------------------------------------
  1. 双击 $($setupExe.Name) 开始安装（按当前用户安装，无需管理员权限）。
  2. 安装最后阶段会把 utils_bundles\ 里约数 GB 的资源解压到安装目录，
     可能需要几分钟，请勿关闭窗口。详细进度会显示在安装器里，并写入安装目录的 install.log。
  3. 若安装器提示「缺少 utils_bundles」，说明 setup.exe 被单独移动了——
     请回到本目录重新运行，保持本目录文件完整。

常见问题
--------------------------------------------------
  * 安装到一半提示解压失败：确认本目录下的 utils_bundles\ 完整（含 bundle-manifest.json
    与全部 .tar.zst），磁盘剩余空间充足，杀毒软件没有拦截写入。
  * 安装卡在解压阶段：点开「显示详细信息」可以看到实时的解包进度。

SHA-256 校验值见发布页的 SHA256SUMS.txt。
"@
Write-Utf8File -Path (Join-Path $stageDir '安装说明.txt') -Content $installNotes -Bom

$stageBytes = [long]((Get-ChildItem -LiteralPath $stageDir -Recurse -File | Measure-Object -Property Length -Sum).Sum)
Write-Host "[package] 解压后体积 $(Format-Size $stageBytes)"

# ---------------------------------------------------------------------------
# 4. 找 7-Zip 并打分卷
# ---------------------------------------------------------------------------
$sevenZip = $null
foreach ($candidate in @(
        'C:\Program Files\7-Zip\7z.exe',
        'C:\Program Files (x86)\7-Zip\7z.exe'
    )) {
    if (Test-Path -LiteralPath $candidate) {
        $sevenZip = $candidate
        break
    }
}
if (-not $sevenZip) {
    $command = Get-Command 7z -ErrorAction SilentlyContinue
    if ($command) {
        $sevenZip = $command.Source
    }
}
if (-not $sevenZip) {
    throw '未找到 7z.exe。GitHub 的 windows-2022 镜像自带 7-Zip；本地请先安装 7-Zip 或把 7z 加进 PATH。'
}

New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
Get-ChildItem -LiteralPath $outputRoot -File |
    Where-Object { $_.Name -like 'guaxi-*-windows-x64-full.7z*' } |
    Remove-Item -Force

$archiveBase = Join-Path $outputRoot "$folderName-full.7z"
Write-Host "[package] 7z 分卷打包（每卷 ${VolumeSizeMb} MiB）…"

Push-Location $stagingRoot
try {
    & $sevenZip a -t7z -mx=1 -mmt=on -ms=off "-v${VolumeSizeMb}m" $archiveBase $folderName
    $sevenZipExit = $LASTEXITCODE
}
finally {
    Pop-Location
}
if ($sevenZipExit -ne 0) {
    throw "7z 打包失败（退出码 $sevenZipExit）"
}

# 总大小不足一卷时 7z 也会产出 .001，这里改回普通 .7z，用户双击即可打开。
$firstPart = "$archiveBase.001"
$secondPart = "$archiveBase.002"
if ((Test-Path -LiteralPath $firstPart) -and -not (Test-Path -LiteralPath $secondPart)) {
    Move-Item -LiteralPath $firstPart -Destination $archiveBase -Force
}

$archiveFiles = @(Get-ChildItem -LiteralPath $outputRoot -File |
        Where-Object { $_.Name -like "$folderName-full.7z*" } |
        Sort-Object Name)

# GitHub 的硬上限是 2 GiB（2,147,483,648 字节）。超了就换更小的 -VolumeSizeMb 重跑。
$assetLimit = 2GB
foreach ($file in $archiveFiles) {
    if ($file.Length -ge $assetLimit) {
        $message = "分卷 {0} 有 {1}，超过 GitHub Release 的 2 GiB 单文件上限。`n请用更小的 -VolumeSizeMb 重新打包。" -f $file.Name, (Format-Size $file.Length)
        throw $message
    }
}

# ---------------------------------------------------------------------------
# 5. SHA-256 与 Release 正文
# ---------------------------------------------------------------------------
$checksums = foreach ($file in $archiveFiles) {
    $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    '{0}  {1}' -f $hash, $file.Name
}
[System.IO.File]::WriteAllLines(
    (Join-Path $outputRoot 'SHA256SUMS.txt'),
    $checksums,
    (New-Object System.Text.UTF8Encoding($false))
)

$partCount = $archiveFiles.Count
$totalArchiveBytes = [long](($archiveFiles | Measure-Object -Property Length -Sum).Sum)
$repoSlug = if ($env:GITHUB_REPOSITORY) { $env:GITHUB_REPOSITORY } else { 'xhcstar/Guaxi' }

$releaseNotes = @"
## Guaxi $Tag · Windows x64 完整离线包

包含安装器、安装期解包工具（``extract_utils_bundles.exe``）与全部运行时资源（``utils_bundles/``，
含语音模型与 RSSHub）。**这些必须放在同一个目录使用，不能只拿 setup.exe。**

> GitHub Release 单个文件上限 2 GiB，完整包解压后约 $(Format-Size $stageBytes)、压缩后 $(Format-Size $totalArchiveBytes)，
> 因此拆成 $partCount 个 7z 分卷发布。

### 安装方法

1. 下载本页**全部** ``$folderName-full.7z.001`` 起的分卷（共 $partCount 个，缺任何一卷都无法解压），放进同一个文件夹。
2. 用 [7-Zip](https://www.7-zip.org/) 或 WinRAR 解压 **.001** 那一个，其余分卷会被自动读取。
3. 进入解压出的 ``$folderName`` 文件夹，双击 ``$($setupExe.Name)`` 安装。
4. 安装最后阶段会解压数 GB 资源，可能需要几分钟，请勿关闭窗口；日志写入安装目录下的 ``install.log``。
5. 校验下载：``SHA256SUMS.txt`` 是每个分卷的 SHA-256。

### How to install

Download **all** ``$folderName-full.7z.00N`` parts into one folder, extract ``.001`` with 7-Zip,
then run ``$($setupExe.Name)`` inside the extracted ``$folderName`` folder. Keep ``utils_bundles/``
and ``extract_utils_bundles.exe`` next to the installer — the installer expands the bundles into the
install directory during setup.

**文件 / Files**

``````text
$((($archiveFiles | ForEach-Object { '{0}  ({1})' -f $_.Name, (Format-Size $_.Length) } | Out-String).TrimEnd()))
``````

**Full Changelog**: https://github.com/$repoSlug/commits/$Tag

---
This release was built automatically by GitHub Actions ($(Get-Date -Format 'yyyy-MM-dd')).
"@
Write-Utf8File -Path $notesFile -Content $releaseNotes

# ---------------------------------------------------------------------------
# 6. 汇总
# ---------------------------------------------------------------------------
Write-Host ''
Write-Host '[package] 完成：'
foreach ($file in $archiveFiles) {
    Write-Host ('  {0,-52} {1}' -f $file.Name, (Format-Size $file.Length))
}
Write-Host ('  {0,-52} {1}' -f 'SHA256SUMS.txt', '')
Write-Host ("[package] 共 {0} 个分卷 · 压缩后 {1} · Release 正文 {2}" -f $partCount, (Format-Size $totalArchiveBytes), $notesFile)
