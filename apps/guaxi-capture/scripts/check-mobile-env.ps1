$ErrorActionPreference = "Continue"

function Test-CommandAvailable {
  param([string]$Name)
  $command = Get-Command $Name -ErrorAction SilentlyContinue
  return $null -ne $command
}

function Write-Check {
  param(
    [string]$Name,
    [bool]$Ok,
    [string]$Detail
  )

  $status = if ($Ok) { "OK" } else { "MISS" }
  Write-Host "[$status] $Name - $Detail"
}

Write-Host "Guaxi Capture mobile build environment check"
Write-Host ""

$javaOk = Test-CommandAvailable "java"
Write-Check "Java" $javaOk $(if ($javaOk) { "java command found" } else { "install JDK and set JAVA_HOME" })

$rustOk = Test-CommandAvailable "rustup"
Write-Check "Rustup" $rustOk $(if ($rustOk) { "rustup command found" } else { "install Rust toolchain" })

if ($rustOk) {
  $targets = rustup target list --installed
  $androidTargets = @(
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "i686-linux-android",
    "x86_64-linux-android"
  )
  foreach ($target in $androidTargets) {
    Write-Check "Rust target $target" ($targets -contains $target) "required for Android builds"
  }
}

$androidHome = $env:ANDROID_HOME
$androidHomeOk = -not [string]::IsNullOrWhiteSpace($androidHome) -and (Test-Path $androidHome)
Write-Check "ANDROID_HOME" $androidHomeOk $(if ($androidHomeOk) { $androidHome } else { "set to Android SDK path, usually C:\Users\$env:USERNAME\AppData\Local\Android\Sdk" })

$ndkHome = $env:NDK_HOME
$ndkHomeOk = -not [string]::IsNullOrWhiteSpace($ndkHome) -and (Test-Path $ndkHome)
Write-Check "NDK_HOME" $ndkHomeOk $(if ($ndkHomeOk) { $ndkHome } else { "set to Android NDK path inside Android SDK\\ndk\\<version>" })

$adbOk = Test-CommandAvailable "adb"
Write-Check "adb" $adbOk $(if ($adbOk) { "adb command found" } else { "add Android SDK platform-tools to PATH" })

$tauriOk = Test-Path "..\..\node_modules\@tauri-apps\cli\tauri.js"
Write-Check "Tauri CLI" $tauriOk $(if ($tauriOk) { "project-local CLI found" } else { "run npm install from repository root or app workspace" })

Write-Host ""
Write-Host "Windows can build Android. iOS builds require macOS + Xcode."
