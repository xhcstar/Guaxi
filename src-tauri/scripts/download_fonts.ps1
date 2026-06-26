param(
  [string]$FontDir = (Join-Path $PSScriptRoot "..\utils\fonts"),
  [string]$CacheDir = (Join-Path $PSScriptRoot "..\target\font-downloads"),
  [switch]$Force
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# 字体资源下载脚本只在需要更新随软件分发的字体时手动运行。
# 设计思路：
# 1. 不放进普通 build 流程，避免每次构建都联网、变慢或受镜像波动影响。
# 2. 只复制运行时真正需要的 ttf/otf 文件到 src-tauri/utils/fonts。
# 3. src-tauri/utils 会被现有 utils_bundles 机制打包，安装器再注册这些字体。
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$fontRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$FontDir = [IO.Path]::GetFullPath($FontDir)
$CacheDir = [IO.Path]::GetFullPath($CacheDir)

New-Item -ItemType Directory -Force -Path $FontDir | Out-Null
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null

$sources = @(
  @{
    Name = "Maple Mono NF CN"
    Url = "https://github.com/subframe7536/maple-font/releases/download/v7.9/MapleMono-NF-CN-unhinted.zip"
    Archive = "MapleMono-NF-CN-unhinted.zip"
    Patterns = @("*.ttf", "*.otf")
    IncludeNameRegex = "MapleMono-NF-CN-(?:Regular|Bold|Italic|BoldItalic)\.(?:ttf|otf)$"
  },
  @{
    Name = "STIX Two Math"
    Url = "https://mirrors.ctan.org/fonts/stix2-otf.zip"
    Archive = "stix2-otf.zip"
    Patterns = @("STIXTwoMath-Regular.otf")
    IncludeNameRegex = "STIXTwoMath-Regular\.otf$"
  }
)

function Download-Archive {
  param(
    [hashtable]$Source,
    [string]$ArchivePath
  )

  if ((Test-Path -LiteralPath $ArchivePath) -and -not $Force) {
    Write-Host "[GuaxiFonts] reuse cached archive: $ArchivePath"
    return
  }

  Write-Host "[GuaxiFonts] downloading $($Source.Name): $($Source.Url)"
  Invoke-WebRequest -Uri $Source.Url -OutFile $ArchivePath -UseBasicParsing -TimeoutSec 600
}

function Expand-FontArchive {
  param(
    [hashtable]$Source,
    [string]$ArchivePath
  )

  $extractDir = Join-Path $CacheDir ([IO.Path]::GetFileNameWithoutExtension($Source.Archive))
  if (Test-Path -LiteralPath $extractDir) {
    Remove-Item -LiteralPath $extractDir -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $extractDir | Out-Null

  Write-Host "[GuaxiFonts] extracting $($Source.Name)"
  Expand-Archive -LiteralPath $ArchivePath -DestinationPath $extractDir -Force

  $regex = [regex]$Source.IncludeNameRegex
  $fonts = Get-ChildItem -LiteralPath $extractDir -Recurse -File |
    Where-Object {
      $_.Extension -in @(".ttf", ".otf") -and $regex.IsMatch($_.Name)
    }

  if (-not $fonts -or $fonts.Count -eq 0) {
    throw "[GuaxiFonts] no matching fonts found for $($Source.Name)"
  }

  foreach ($font in $fonts) {
    $target = Join-Path $FontDir $font.Name
    Copy-Item -LiteralPath $font.FullName -Destination $target -Force
    Write-Host "[GuaxiFonts] bundled font: $($font.Name)"
  }
}

foreach ($source in $sources) {
  $archivePath = Join-Path $CacheDir $source.Archive
  Download-Archive -Source $source -ArchivePath $archivePath
  Expand-FontArchive -Source $source -ArchivePath $archivePath
}

Write-Host "[GuaxiFonts] font assets are ready: $FontDir"
