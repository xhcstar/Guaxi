param(
  [string]$FontDir = $PSScriptRoot
)

$ErrorActionPreference = "Stop"

# 安装器在解压 utils_bundles 后调用本脚本。
# 设计思路：
# 1. 当前安装包是 currentUser 模式，所以字体只写入当前用户字体目录，避免管理员权限要求。
# 2. 字体是可选体验资源，安装失败不能影响主程序安装，因此每个字体单独捕获错误并继续。
# 3. 先检查 HKCU/HKLM 已安装字体族；系统或用户已经有同族字体时直接跳过，绝不覆盖。
# 4. 不再使用 Guaxi 前缀重命名字体文件或注册表项，避免破坏 MobaXterm 等软件的字体枚举。
# 5. 会清理旧版脚本误写入的 Guaxi 前缀记录，但只处理 HKCU 中指向 Guaxi-* 用户字体文件的 Guaxi 注册表项。
# 6. 复制到用户字体目录时使用原始文件名；目标文件已存在则跳过，避免破坏用户已有字体。
# 7. 安装或清理后广播 WM_FONTCHANGE，让 WebView / Monaco / 终端类软件尽快刷新字体缓存。
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

    try {
      $item = Get-ItemProperty -LiteralPath $path
      foreach ($property in $item.PSObject.Properties) {
        if ($property.Name -like "PS*") {
          continue
        }
        [void]$names.Add([string]$property.Name)
      }
    } catch {
      Write-Host "[GuaxiFonts] failed to read font registry names path=$path error=$($_.Exception.Message)"
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
      Write-Host "[GuaxiFonts] failed to read font registry values path=$path error=$($_.Exception.Message)"
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

function Test-RegistryValueExists {
  param(
    [string]$RegPath,
    [string]$Name
  )

  $existing = Get-ItemProperty -LiteralPath $RegPath -Name $Name -ErrorAction SilentlyContinue
  return ($null -ne $existing)
}

function Remove-GuaxiPrefixedFontInstall {
  param(
    [string]$RegPath,
    [string]$UserFontDir
  )

  $removedEntries = 0
  $removedFiles = 0
  if (-not (Test-Path -LiteralPath $RegPath)) {
    return [pscustomobject]@{
      RemovedEntries = $removedEntries
      RemovedFiles = $removedFiles
    }
  }

  $userFontRoot = [IO.Path]::GetFullPath($UserFontDir).TrimEnd('\') + '\'

  try {
    $item = Get-ItemProperty -LiteralPath $RegPath
    foreach ($property in $item.PSObject.Properties) {
      if ($property.Name -like "PS*") {
        continue
      }
      if ($property.Name -notlike "Guaxi *") {
        continue
      }

      $value = [string]$property.Value
      $fileName = ""
      $isGuaxiUserFontRecord = $false
      if ($value) {
        try {
          $fullValuePath = [IO.Path]::GetFullPath($value)
          $fileName = [IO.Path]::GetFileName($fullValuePath)
          $isUnderUserFonts = $fullValuePath.StartsWith($userFontRoot, [System.StringComparison]::OrdinalIgnoreCase)
          $isGuaxiUserFontRecord = ($isUnderUserFonts -and $fileName -like "Guaxi-*")
        } catch {
          Write-Host "[GuaxiFonts] Guaxi registry value is not a normal path, registryName=$($property.Name) value=$value"
        }
      }

      if (-not $isGuaxiUserFontRecord) {
        Write-Host "[GuaxiFonts] keep Guaxi-like registry entry because it was not created by old installer logic name=$($property.Name) value=$value"
        continue
      }

      Remove-ItemProperty -LiteralPath $RegPath -Name $property.Name -ErrorAction Stop
      $removedEntries += 1
      Write-Host "[GuaxiFonts] removed old Guaxi-prefixed registry entry name=$($property.Name) value=$value"

      if (Test-Path -LiteralPath $value) {
        Remove-Item -LiteralPath $value -Force -ErrorAction Stop
        $removedFiles += 1
        Write-Host "[GuaxiFonts] removed old Guaxi-prefixed font file file=$fileName"
      } else {
        Write-Host "[GuaxiFonts] old Guaxi-prefixed font file already missing file=$fileName"
      }
    }
  } catch {
    Write-Host "[GuaxiFonts] failed while cleaning old Guaxi-prefixed font install error=$($_.Exception.Message)"
  }

  return [pscustomobject]@{
    RemovedEntries = $removedEntries
    RemovedFiles = $removedFiles
  }
}

function Send-FontChangeBroadcast {
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
}

$userFontDir = Join-Path $env:LOCALAPPDATA "Microsoft\Windows\Fonts"
New-Item -ItemType Directory -Force -Path $userFontDir | Out-Null

$regPath = "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"
New-Item -Path $regPath -Force | Out-Null

$cleanupResult = Remove-GuaxiPrefixedFontInstall -RegPath $regPath -UserFontDir $userFontDir
$cleanupChanged = ($cleanupResult.RemovedEntries -gt 0 -or $cleanupResult.RemovedFiles -gt 0)

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

    $dest = Join-Path $userFontDir $font.Name
    if (Test-Path -LiteralPath $dest) {
      Write-Host "[GuaxiFonts] destination file already exists, skip to avoid overwriting user font file=$($font.Name) dest=$dest"
      $skippedCount += 1
      continue
    }

    $fontKind = if ($font.Extension -ieq ".otf") { "OpenType" } else { "TrueType" }
    $valueName = "$([IO.Path]::GetFileNameWithoutExtension($font.Name)) ($fontKind)"
    if (Test-RegistryValueExists -RegPath $regPath -Name $valueName) {
      Write-Host "[GuaxiFonts] registry value already exists, skip to avoid replacing font registration name=$valueName file=$($font.Name)"
      $skippedCount += 1
      continue
    }

    Copy-Item -LiteralPath $font.FullName -Destination $dest
    New-ItemProperty -Path $regPath -Name $valueName -Value $dest -PropertyType String | Out-Null

    $added = [GuaxiFontNative]::AddFontResourceW($dest)
    Write-Host "[GuaxiFonts] installed file=$($font.Name) dest=$([IO.Path]::GetFileName($dest)) registryName=$valueName family=$family added=$added"
    [void]$seenFamilies.Add($family)
    $installedCount += 1
  } catch {
    $failedCount += 1
    Write-Host "[GuaxiFonts] failed to install file=$($font.Name) error=$($_.Exception.Message)"
  }
}

if ($installedCount -gt 0 -or $cleanupChanged) {
  Send-FontChangeBroadcast
  Write-Host "[GuaxiFonts] broadcast WM_FONTCHANGE after font changes."
}

Write-Host "[GuaxiFonts] font install finished. installed=$installedCount skipped=$skippedCount failed=$failedCount cleanedEntries=$($cleanupResult.RemovedEntries) cleanedFiles=$($cleanupResult.RemovedFiles) families=$($seenFamilies.Count)"
exit 0
