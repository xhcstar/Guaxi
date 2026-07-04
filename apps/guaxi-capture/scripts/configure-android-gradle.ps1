param(
  [string]$BaseUrl = "https://mirrors.cloud.tencent.com/gradle",
  [string]$ProjectRoot = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
  $ProjectRoot = Split-Path -Parent $PSScriptRoot
}

$wrapperProperties = Join-Path $ProjectRoot "src-tauri\gen\android\gradle\wrapper\gradle-wrapper.properties"

if (!(Test-Path $wrapperProperties)) {
  Write-Host "[capture-android] gradle-wrapper.properties not found: $wrapperProperties"
  Write-Host "[capture-android] Run npm run android:init first, then rerun this script."
  exit 1
}

$content = Get-Content -Encoding UTF8 $wrapperProperties
$distributionLine = $content | Where-Object { $_ -match '^distributionUrl=' } | Select-Object -First 1

if ([string]::IsNullOrWhiteSpace($distributionLine)) {
  Write-Host "[capture-android] distributionUrl not found in $wrapperProperties"
  exit 1
}

$fileName = Split-Path ($distributionLine -replace '^distributionUrl=', '') -Leaf
if ([string]::IsNullOrWhiteSpace($fileName) -or $fileName -notmatch '^gradle-.+\.zip$') {
  Write-Host "[capture-android] cannot parse Gradle distribution file from: $distributionLine"
  exit 1
}

$newUrl = "$($BaseUrl.TrimEnd('/'))/$fileName"
$escapedUrl = $newUrl -replace ':', '\:'

# 设计说明：
# Tauri 生成的 Android 工程使用 Gradle Wrapper。国内网络下载 services.gradle.org
# 经常超时，所以这里只替换 wrapper 的 distributionUrl，不改动业务代码和 Gradle 版本。
$updated = @()
foreach ($line in $content) {
  if ($line -match '^distributionUrl=') {
    $updated += "distributionUrl=$escapedUrl"
    continue
  }

  $updated += $line
}

Set-Content -Encoding UTF8 $wrapperProperties $updated

Write-Host "[capture-android] Gradle wrapper distributionUrl updated:"
Write-Host "[capture-android] $newUrl"
