param(
  [string]$FontDir = $PSScriptRoot
)

$ErrorActionPreference = "Stop"

# 安装器在解压 utils_bundles 后调用本脚本。
# 设计思路：
# 1. 当前安装包是 currentUser 模式，所以字体也写入用户字体目录，避免管理员权限。
# 2. 字体是可选体验资源，安装失败不能影响主程序安装，因此脚本尽量记录问题并继续。
# 3. 先检查 HKCU/HKLM 已安装字体族；系统已有则跳过，避免重复注册。
# 4. 复制到用户字体目录时绝不覆盖既有同名文件，避免破坏用户或旧版本已经安装的字体。
# 5. 注册后广播 WM_FONTCHANGE，让 WebView / Monaco / Tectonic 能尽快看到新字体。
Write-Host "[GuaxiFonts] font install started. fontDir=$FontDir"

if (-not (Test-Path -LiteralPath $FontDir)) {
  Write-Host "[GuaxiFonts] font directory does not exist, skip."
  exit 0
}

$fontFiles = Get-ChildItem -LiteralPath $FontDir -File |
  Where-Object { $_.Extension -in @(".ttf", ".otf") }

if (-not $fontFiles -or $fontFiles.Count -eq 0) {
  Write-Host "[GuaxiFonts] no bundled font files found, skip."
  exit 0
}

Add-Type -AssemblyName System.Drawing
if (-not ("GuaxiFontNative" -as [type])) {
  Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class GuaxiFontNative {
  [DllImport("gdi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern int AddFontResourceW(string lpszFilename);

  [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
  public static extern IntPtr SendMessageTimeout(
    IntPtr hWnd,
    uint Msg,
    UIntPtr wParam,
    IntPtr lParam,
    uint fuFlags,
    uint uTimeout,
    out UIntPtr lpdwResult
  );
}
"@
}

function Get-InstalledFontRegistryNames {
  $paths = @(
    "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts",
    "HKLM:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"
  )

  $names = New-Object System.Collections.Generic.List[string]
  foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path)) {
      continue
    }

    $item = Get-ItemProperty -LiteralPath $path
    foreach ($property in $item.PSObject.Properties) {
      if ($property.Name -like "PS*") {
        continue
      }
      [void]$names.Add([string]$property.Name)
    }
  }
  return $names
}

function Get-InstalledFontRegistryValues {
  $paths = @(
    "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts",
    "HKLM:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"
  )

  $values = New-Object System.Collections.Generic.List[object]
  foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path)) {
      continue
    }

    try {
      $item = Get-ItemProperty -LiteralPath $path
      foreach ($property in $item.PSObject.Properties) {
        if ($property.Name -like "PS*") {
          continue
        }
        $values.Add([pscustomobject]@{
          Path = $path
          Name = [string]$property.Name
          Value = [string]$property.Value
        }) | Out-Null
      }
    } catch {
      Write-Host "[GuaxiFonts] failed to read font registry path=$path error=$($_.Exception.Message)"
    }
  }
  return $values
}

function Get-FontFamilyName {
  param([string]$Path)

  $collection = New-Object System.Drawing.Text.PrivateFontCollection
  try {
    $collection.AddFontFile($Path)
    if ($collection.Families.Count -gt 0) {
      return $collection.Families[0].Name
    }
  } catch {
    Write-Host "[GuaxiFonts] failed to inspect font family, fallback to filename: $Path; error=$($_.Exception.Message)"
  } finally {
    $collection.Dispose()
  }

  return [IO.Path]::GetFileNameWithoutExtension($Path)
}

function Test-FamilyAlreadyInstalled {
  param(
    [string]$Family,
    [string[]]$RegistryNames
  )

  foreach ($name in $RegistryNames) {
    if ($name -like "$Family*") {
      return $true
    }
  }
  return $false
}

function Test-RegistryAlreadyHasFile {
  param(
    [string]$FileName,
    [object[]]$RegistryValues
  )

  foreach ($entry in $RegistryValues) {
    $value = [string]$entry.Value
    if (-not $value) {
      continue
    }
    if ([IO.Path]::GetFileName($value) -ieq $FileName) {
      return $true
    }
  }
  return $false
}

function Get-SafeFontDestination {
  param(
    [System.IO.FileInfo]$Font,
    [string]$UserFontDir
  )

  # 使用 Guaxi 前缀避免与用户已经安装的同名字体文件冲突。
  $destName = "Guaxi-$($Font.Name)"
  $dest = Join-Path $UserFontDir $destName
  if (-not (Test-Path -LiteralPath $dest)) {
    return $dest
  }

  try {
    $srcHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Font.FullName).Hash
    $destHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $dest).Hash
    if ($srcHash -eq $destHash) {
      Write-Host "[GuaxiFonts] same Guaxi font file already exists, reuse path=$dest"
      return $dest
    }

    $shortHash = $srcHash.Substring(0, 12).ToLowerInvariant()
    $stem = [IO.Path]::GetFileNameWithoutExtension($Font.Name)
    $ext = $Font.Extension
    return (Join-Path $UserFontDir "Guaxi-$stem-$shortHash$ext")
  } catch {
    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $stem = [IO.Path]::GetFileNameWithoutExtension($Font.Name)
    $ext = $Font.Extension
    Write-Host "[GuaxiFonts] failed to hash existing font destination, fallback to timestamp. file=$($Font.Name) error=$($_.Exception.Message)"
    return (Join-Path $UserFontDir "Guaxi-$stem-$stamp$ext")
  }
}

$userFontDir = Join-Path $env:LOCALAPPDATA "Microsoft\Windows\Fonts"
New-Item -ItemType Directory -Force -Path $userFontDir | Out-Null

$regPath = "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"
New-Item -Path $regPath -Force | Out-Null

$preinstalledRegistryNames = @(Get-InstalledFontRegistryNames)
$preinstalledRegistryValues = @(Get-InstalledFontRegistryValues)
$installedCount = 0
$skippedCount = 0
$failedCount = 0
$seenFamilies = New-Object System.Collections.Generic.HashSet[string]

foreach ($font in $fontFiles) {
  try {
    $family = Get-FontFamilyName -Path $font.FullName
    $alreadyInstalled = Test-FamilyAlreadyInstalled -Family $family -RegistryNames $preinstalledRegistryNames
    if ($alreadyInstalled) {
      Write-Host "[GuaxiFonts] already installed, skip family=$family file=$($font.Name)"
      $skippedCount += 1
      continue
    }

    if (Test-RegistryAlreadyHasFile -FileName $font.Name -RegistryValues $preinstalledRegistryValues) {
      Write-Host "[GuaxiFonts] registry already references same file name, skip file=$($font.Name)"
      $skippedCount += 1
      continue
    }

    $dest = Get-SafeFontDestination -Font $font -UserFontDir $userFontDir
    if (-not (Test-Path -LiteralPath $dest)) {
      Copy-Item -LiteralPath $font.FullName -Destination $dest
    }

    $fontKind = if ($font.Extension -ieq ".otf") { "OpenType" } else { "TrueType" }
    $valueName = "Guaxi $([IO.Path]::GetFileNameWithoutExtension([IO.Path]::GetFileName($dest))) ($fontKind)"
    New-ItemProperty -Path $regPath -Name $valueName -Value $dest -PropertyType String -Force | Out-Null

    $added = [GuaxiFontNative]::AddFontResourceW($dest)
    Write-Host "[GuaxiFonts] installed file=$($font.Name) dest=$([IO.Path]::GetFileName($dest)) family=$family added=$added"
    [void]$seenFamilies.Add($family)
    $installedCount += 1
  } catch {
    $failedCount += 1
    Write-Host "[GuaxiFonts] failed to install file=$($font.Name) error=$($_.Exception.Message)"
  }
}

if ($installedCount -gt 0) {
  $HWND_BROADCAST = [IntPtr]0xffff
  $WM_FONTCHANGE = 0x001D
  $SMTO_ABORTIFHUNG = 0x0002
  $result = [UIntPtr]::Zero
  [void][GuaxiFontNative]::SendMessageTimeout(
    $HWND_BROADCAST,
    $WM_FONTCHANGE,
    [UIntPtr]::Zero,
    [IntPtr]::Zero,
    $SMTO_ABORTIFHUNG,
    5000,
    [ref]$result
  )
  Write-Host "[GuaxiFonts] broadcast WM_FONTCHANGE after installing fonts."
}

Write-Host "[GuaxiFonts] font install finished. installed=$installedCount skipped=$skippedCount failed=$failedCount families=$($seenFamilies.Count)"
exit 0
