param(
  [string]$BaseUrl = "https://mirrors.cloud.tencent.com/gradle",
  [string]$GradleVersion = "8.14.3",
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
$content = $content | ForEach-Object { $_ -replace "^\uFEFF", "" }
$distributionLine = $content | Where-Object { $_ -match '^distributionUrl=' } | Select-Object -First 1

if ([string]::IsNullOrWhiteSpace($distributionLine)) {
  Write-Host "[capture-android] distributionUrl not found, repairing with Gradle $GradleVersion"
  $distributionLine = "distributionUrl=https://services.gradle.org/distributions/gradle-$GradleVersion-bin.zip"
  $content += $distributionLine
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

# 关键：Gradle Wrapper 使用 Java Properties 读取该文件。
# Windows PowerShell 5.1 的 Set-Content -Encoding UTF8 会写入 BOM，
# Java 会把第一行解析成带隐藏字符的 key，导致报 “No value with key 'distributionUrl'”。
# 因此这里必须使用 UTF-8 no BOM 写回。
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllLines($wrapperProperties, $updated, $utf8NoBom)

Write-Host "[capture-android] Gradle wrapper distributionUrl updated:"
Write-Host "[capture-android] $newUrl"
