param(
  [string]$BaseUrl = "https://mirrors.cloud.tencent.com/gradle",
  [string]$GradleVersion = "8.14.3",
  [string]$ProjectRoot = ""
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$nodeScript = Join-Path $scriptRoot "run-android-gradle-config.mjs"

# Design note:
# The actual Gradle wrapper rewrite now lives in configure-android-gradle.sh.
# This compatibility entry keeps old manual PowerShell commands working while
# avoiding PowerShell 5.1 UTF-8 BOM behavior for Java properties files.
$nodeArgs = @(
  $nodeScript,
  "--base-url",
  $BaseUrl,
  "--gradle-version",
  $GradleVersion
)

if (![string]::IsNullOrWhiteSpace($ProjectRoot)) {
  $nodeArgs += "--project-root"
  $nodeArgs += $ProjectRoot
}

Write-Host "[capture-android] PowerShell wrapper is deprecated; running shell-based configurator."
& node @nodeArgs
exit $LASTEXITCODE
