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
# 5. 复制到用户字体目录时使用原始文件名；目标文件已存在则跳过，避免破坏用户已有字体。
# 6. 安装或清理后广播 WM_FONTCHANGE，让 WebView / Monaco / 终端类软件尽快刷新字体缓存。
#
# 关于旧版 Guaxi-* 记录的迁移（重要 · 修复“机器上已装的字体被安装脚本删掉”）：
# 7. 旧版脚本会把字体改名成 Guaxi-*.ttf 并以 "Guaxi xxx" 注册。之前的清理逻辑在安装之前就
#    先把这些记录连文件一起删掉，于是“机器上本来已经装好的字体”会先消失；一旦随后的安装
#    步骤失败（磁盘、权限、注册表任意一步），字体就彻底没了。
# 8. 现在改为“先安装、后清理，且只在确认有替代品之后才清理”：
#    - 判断“是否已安装”时忽略旧版 Guaxi-* 记录，否则永远无法迁移；
#    - 安装成功后，才允许删除同一字体族的旧版 Guaxi 注册表项；
#    - 旧版字体文件只有在“注册表项已删除 + 位于用户字体目录 + 文件名是 Guaxi-* + 没有任何
#      其它注册表项还在引用它”这四个条件同时满足时才删除。
#    这样无论中间哪一步失败，机器上的字体都不会凭空少一个。
# 9. 除上述旧版 Guaxi-* 产物外，本脚本不会删除机器上任何字体文件或注册表项。
#
# 关于“重装/卸载后 Maple Mono 找不到”（重要 · 已修）：
# 10. 旧版脚本用 `New-Item -Path HKCU:\...\Fonts -Force` 来“确保字体注册表项存在”。PowerShell
#     的注册表提供程序在项已存在时，会先清空该项的所有值再重建。于是安装脚本每次运行都会把
#     当前用户 HKCU 下的**全部按用户字体注册记录**抹掉（包括用户自己装的所有字体）。
# 11. 更糟的是脚本随后按“注册表里有没有”判断“是否已安装”，而这份快照是在清空之后读的，
#     于是判定为“没装过”，可接着又发现字体文件早已躺在用户字体目录里 → 走“目标文件已存在，
#     跳过”的分支，只复制不注册。结果：字体文件在、注册记录没了，Windows 与 WebView 都不再
#     认识这个字体，界面就退化成兜底字体。这正是“首次安装正常、卸载或重装后字体消失”的原因。
#     现在：注册表项只在不存在时创建；并且每次安装都会复核“文件在但没注册”的情况并补注册。
#
# 【本文件必须保存为带 BOM 的 UTF-8】installer-hooks.nsh 用 `powershell.exe -File` 执行它，
# 而 PowerShell 5.1 对没有 BOM 的文件按系统 ANSI 代码页解码。中文 Windows（代码页 936）下
# 本文件的中文注释会被解成乱码，解析在第一个函数处就失败——脚本一行都不会执行，字体自然
# 装不上（安装器只会弹一句“注册 Guaxi 字体失败”）。用不支持 BOM 的编辑器改写本文件前请确认
# 保存时保留 BOM。
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

  [DllImport("gdi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern bool RemoveFontResourceW(string lpszFilename);

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

function Get-CompactFontKey {
  param([string]$Text)

  if (-not $Text) {
    return ""
  }
  # "MapleMono-NF-CN-Regular (TrueType)" 与 "Maple Mono NF CN" 指的是同一个字体族，
  # 但字面上对不上。这里统一压成小写字母数字串再比较，避免把机器上已有的字体判成“未安装”
  # 而重复装一份。
  $withoutKind = [regex]::Replace($Text, "\s*\((TrueType|OpenType)\)\s*$", "")
  return ([regex]::Replace($withoutKind, "[^0-9A-Za-z]", "")).ToLowerInvariant()
}

function Test-IsLegacyGuaxiFontRecord {
  param(
    [object]$Entry,
    [string]$UserFontRoot
  )

  # 旧版脚本的产物特征：注册表项名以 "Guaxi " 开头，且值指向用户字体目录下的 Guaxi-*.ttf。
  # 三个条件必须同时满足，才认定是“我们自己以前写坏的记录”，其余一律视为用户资产。
  if ([string]$Entry.Name -notlike "Guaxi *") {
    return $false
  }
  $value = [string]$Entry.Value
  if (-not $value) {
    return $false
  }
  try {
    $fullValuePath = [IO.Path]::GetFullPath($value)
  } catch {
    return $false
  }
  if (-not $fullValuePath.StartsWith($UserFontRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $false
  }
  return ([IO.Path]::GetFileName($fullValuePath) -like "Guaxi-*")
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

function Test-FontRegistryValueFileExists {
  param([string]$Value)

  # 注册表里字体值有两种写法：HKLM 的系统字体往往只有文件名（相对 %WINDIR%\Fonts，甚至是
  # 8.3 短名），HKCU 里是本脚本或系统写进去的绝对路径。两种都要判断。
  if (-not $Value) {
    return $false
  }

  try {
    $path = if ([IO.Path]::IsPathRooted($Value)) {
      $Value
    } else {
      Join-Path (Join-Path $env:WINDIR "Fonts") $Value
    }
    return (Test-Path -LiteralPath $path -PathType Leaf)
  } catch {
    return $false
  }
}

function Get-AvailableFontRegistryEntry {
  param(
    [string]$Family,
    [string]$FileName,
    [object[]]$RegistryEntries
  )

  # “机器上已经有这个字体”必须同时满足两点：注册表里有对应记录，且记录指向的字体文件真实存在。
  # 只看记录名会把两类损坏现场误判成“已安装”：记录还在但文件被删（用户在系统设置里卸载过、
  # 清理工具删过），以及记录被旧版安装脚本清空过。一旦误判成“已安装”就会跳过安装，字体再也
  # 修不回来——重装后界面字体退化正是这么来的。
  #
  # 只用“非旧版 Guaxi 记录”判断是否已安装：否则一台只装过旧版的机器会被判成“已安装”，
  # 从而跳过按正常命名的安装，旧记录也就永远迁移不掉。
  $familyKey = Get-CompactFontKey -Text $Family
  $fileKey = Get-CompactFontKey -Text ([IO.Path]::GetFileNameWithoutExtension($FileName))

  foreach ($entry in $RegistryEntries) {
    $nameKey = Get-CompactFontKey -Text ([string]$entry.Name)
    if (-not $nameKey) {
      continue
    }
    if (-not (($familyKey -and $nameKey.StartsWith($familyKey)) -or ($fileKey -and $nameKey -eq $fileKey))) {
      continue
    }
    if (Test-FontRegistryValueFileExists -Value ([string]$entry.Value)) {
      return $entry
    }
    Write-Host "[GuaxiFonts] registry entry points to a missing font file, treat as not installed name=$($entry.Name) value=$($entry.Value)"
  }
  return $null
}

function Test-RegistryAlreadyHasFile {
  param(
    [string]$FileName,
    [object[]]$RegistryValues
  )

  # 同样要求文件真实存在：否则一条指向已删除文件的旧记录会让本次安装永远跳过复制与注册。
  foreach ($entry in $RegistryValues) {
    $value = [string]$entry.Value
    if (-not $value) {
      continue
    }
    if ([IO.Path]::GetFileName($value) -ine $FileName) {
      continue
    }
    if (Test-FontRegistryValueFileExists -Value $value) {
      return $true
    }
    Write-Host "[GuaxiFonts] registry entry references a missing file, treat as not registered name=$($entry.Name) value=$value"
  }
  return $false
}

function Get-RegistryValuePath {
  param(
    [string]$RegPath,
    [string]$Name
  )

  $existing = Get-ItemProperty -LiteralPath $RegPath -Name $Name -ErrorAction SilentlyContinue
  if ($null -eq $existing) {
    return ""
  }
  return [string]$existing.$Name
}

function Remove-LegacyGuaxiFontRecords {
  param(
    [string]$RegPath,
    [string]$UserFontDir,
    [string[]]$CoveredFamilyKeys
  )

  # 迁移旧版 Guaxi-* 记录。安全前提（缺一不可）：
  # 1. 只处理“注册表项名 Guaxi * + 值在用户字体目录 + 文件名 Guaxi-*”的记录；
  # 2. 该字体族必须已经有一份按正常命名注册好的替代品（CoveredFamilyKeys），否则原样保留；
  # 3. 字体文件只有在没有任何其它注册表项还引用它时才删除。
  # 任一条不满足就跳过——宁可留下一条旧记录，也不能让机器上的字体凭空消失。
  $removedEntries = 0
  $removedFiles = 0
  $result = [pscustomobject]@{
    RemovedEntries = $removedEntries
    RemovedFiles = $removedFiles
  }
  if (-not (Test-Path -LiteralPath $RegPath)) {
    return $result
  }
  if (-not $CoveredFamilyKeys -or $CoveredFamilyKeys.Count -eq 0) {
    Write-Host "[GuaxiFonts] no newly registered family, keep all legacy Guaxi records untouched"
    return $result
  }

  $userFontRoot = [IO.Path]::GetFullPath($UserFontDir).TrimEnd('\') + '\'
  $covered = New-Object System.Collections.Generic.HashSet[string]
  foreach ($key in $CoveredFamilyKeys) {
    if ($key) { [void]$covered.Add($key) }
  }

  try {
    $currentEntries = @(Get-InstalledFontRegistryValues)
    $legacyEntries = @($currentEntries | Where-Object {
      Test-IsLegacyGuaxiFontRecord -Entry $_ -UserFontRoot $userFontRoot
    })

    foreach ($entry in $legacyEntries) {
      $value = [string]$entry.Value
      $fileName = [IO.Path]::GetFileName($value)
      # 旧记录名形如 "Guaxi MapleMono-NF-CN-Regular (TrueType)"，去掉 Guaxi 前缀后压成比较键。
      $legacyKey = Get-CompactFontKey -Text ([string]$entry.Name).Substring(6)
      $isCovered = $false
      foreach ($key in $covered) {
        if ($legacyKey -and ($legacyKey.StartsWith($key) -or $key.StartsWith($legacyKey))) {
          $isCovered = $true
          break
        }
      }

      if (-not $isCovered) {
        Write-Host "[GuaxiFonts] keep legacy Guaxi record because no replacement was installed name=$($entry.Name) value=$value"
        continue
      }

      Remove-ItemProperty -LiteralPath $entry.Path -Name $entry.Name -ErrorAction Stop
      $removedEntries += 1
      Write-Host "[GuaxiFonts] migrated legacy Guaxi registry entry name=$($entry.Name) value=$value"

      $stillReferenced = @($currentEntries | Where-Object {
        $_.Name -ne $entry.Name -and $_.Value -and ([IO.Path]::GetFileName([string]$_.Value) -ieq $fileName)
      }).Count -gt 0
      if ($stillReferenced) {
        Write-Host "[GuaxiFonts] keep legacy font file because another registry entry still references it file=$fileName"
        continue
      }

      if (Test-Path -LiteralPath $value) {
        [void][GuaxiFontNative]::RemoveFontResourceW($value)
        Remove-Item -LiteralPath $value -Force -ErrorAction Stop
        $removedFiles += 1
        Write-Host "[GuaxiFonts] removed legacy Guaxi-prefixed font file file=$fileName"
      } else {
        Write-Host "[GuaxiFonts] legacy Guaxi-prefixed font file already missing file=$fileName"
      }
    }
  } catch {
    Write-Host "[GuaxiFonts] failed while migrating legacy Guaxi font records error=$($_.Exception.Message)"
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
# 文件系统提供程序的 -Force 是幂等的（已存在时原样返回、不清内容），这里可以放心用。
New-Item -ItemType Directory -Force -Path $userFontDir | Out-Null

$regPath = "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"
# 绝不能用 `New-Item -Path $regPath -Force` 来“确保项存在”：注册表提供程序在项已存在时会先
# 清空该项的所有值再重建，等于把当前用户的全部按用户字体注册记录（含用户自己装的字体）一次
# 抹干净。只在项确实不存在时才创建。
if (-not (Test-Path -LiteralPath $regPath)) {
  New-Item -Path $regPath -Force | Out-Null
}

$userFontRoot = [IO.Path]::GetFullPath($userFontDir).TrimEnd('\') + '\'
$preinstalledRegistryValues = @(Get-InstalledFontRegistryValues)
# 判断“机器上是否已经有这个字体”时排除旧版 Guaxi-* 记录，其余记录一律视为用户资产。
$userOwnedRegistryValues = @($preinstalledRegistryValues | Where-Object {
  -not (Test-IsLegacyGuaxiFontRecord -Entry $_ -UserFontRoot $userFontRoot)
})
$installedCount = 0
$repairedCount = 0
$skippedCount = 0
$failedCount = 0
$seenFamilies = New-Object System.Collections.Generic.HashSet[string]
$installedFamilyKeys = New-Object System.Collections.Generic.List[string]

foreach ($font in $fontFiles) {
  try {
    $family = Get-FontFamilyName -Path $font.FullName
    $dest = Join-Path $userFontDir $font.Name
    $fontKind = if ($font.Extension -ieq ".otf") { "OpenType" } else { "TrueType" }
    $valueName = "$([IO.Path]::GetFileNameWithoutExtension($font.Name)) ($fontKind)"

    $availableEntry = Get-AvailableFontRegistryEntry -Family $family -FileName $font.Name -RegistryEntries $userOwnedRegistryValues
    if ($availableEntry) {
      Write-Host "[GuaxiFonts] already installed, skip family=$family file=$($font.Name) registryName=$($availableEntry.Name)"
      $skippedCount += 1
      continue
    }

    if (Test-RegistryAlreadyHasFile -FileName $font.Name -RegistryValues $userOwnedRegistryValues) {
      Write-Host "[GuaxiFonts] registry already references same file name, skip file=$($font.Name)"
      $skippedCount += 1
      continue
    }

    # 同名注册项如果已经指向一个确实存在的别的字体文件，就不抢这个名字，避免破坏别人的注册。
    $existingValue = Get-RegistryValuePath -RegPath $regPath -Name $valueName
    if ($existingValue -and ($existingValue -ine $dest) -and (Test-FontRegistryValueFileExists -Value $existingValue)) {
      Write-Host "[GuaxiFonts] registry name is already used by another installed font, skip file=$($font.Name) registryName=$valueName value=$existingValue"
      $skippedCount += 1
      continue
    }

    if (Test-Path -LiteralPath $dest) {
      # 文件已经躺在用户字体目录里，但没有任何可用记录指向它（旧版安装脚本清空过 HKCU 字体
      # 注册表，或用户在系统设置里卸载后文件没删干净）。这种情况只要补注册就能修好；早期版本
      # 在这里直接跳过，于是字体永远回不来。这里不复制文件，只补注册。
      New-ItemProperty -Path $regPath -Name $valueName -Value $dest -PropertyType String -Force | Out-Null
      $added = [GuaxiFontNative]::AddFontResourceW($dest)
      Write-Host "[GuaxiFonts] re-registered existing font file=$($font.Name) dest=$dest registryName=$valueName previousValue=$existingValue added=$added"
      $repairedCount += 1
    } else {
      Copy-Item -LiteralPath $font.FullName -Destination $dest
      New-ItemProperty -Path $regPath -Name $valueName -Value $dest -PropertyType String -Force | Out-Null
      $added = [GuaxiFontNative]::AddFontResourceW($dest)
      Write-Host "[GuaxiFonts] installed file=$($font.Name) dest=$dest registryName=$valueName family=$family added=$added"
      $installedCount += 1
    }

    [void]$seenFamilies.Add($family)
    # 只有“确实装成功”的字体族才有资格让对应的旧版 Guaxi 记录被清理。
    $installedFamilyKeys.Add((Get-CompactFontKey -Text ([IO.Path]::GetFileNameWithoutExtension($font.Name))))
    $installedFamilyKeys.Add((Get-CompactFontKey -Text $family))
  } catch {
    $failedCount += 1
    Write-Host "[GuaxiFonts] failed to install file=$($font.Name) error=$($_.Exception.Message)"
  }
}

# 安装完成之后才迁移旧版记录：此时替代品已经就位，删除旧记录不会让字体消失。
$cleanupResult = Remove-LegacyGuaxiFontRecords -RegPath $regPath -UserFontDir $userFontDir -CoveredFamilyKeys @($installedFamilyKeys)
$cleanupChanged = ($cleanupResult.RemovedEntries -gt 0 -or $cleanupResult.RemovedFiles -gt 0)

if ($installedCount -gt 0 -or $repairedCount -gt 0 -or $cleanupChanged) {
  Send-FontChangeBroadcast
  Write-Host "[GuaxiFonts] broadcast WM_FONTCHANGE after font changes."
}

# 收尾复核：每个随软件分发的字体族是否真的已经可用。
# 字体装不上时软件界面会退化成兜底字体，而安装日志里只有脚本的 stdout，这行是现场唯一的线索；
# 复核失败时返回非 0，让安装器弹出“字体注册失败”的提示，而不是静默装成一个字体不可用的版本。
$postInstallValues = @(Get-InstalledFontRegistryValues)
$verifiedFamilies = New-Object System.Collections.Generic.List[string]
$unavailableFamilies = New-Object System.Collections.Generic.List[string]
foreach ($font in $fontFiles) {
  $family = Get-FontFamilyName -Path $font.FullName
  if ($verifiedFamilies.Contains($family)) {
    continue
  }
  [void]$verifiedFamilies.Add($family)
  $entry = Get-AvailableFontRegistryEntry -Family $family -FileName $font.Name -RegistryEntries $postInstallValues
  if ($entry) {
    Write-Host "[GuaxiFonts] verified available family=$family registryName=$($entry.Name)"
  } else {
    [void]$unavailableFamilies.Add($family)
    Write-Host "[GuaxiFonts] WARNING font family is NOT available after install family=$family file=$($font.Name)"
  }
}

Write-Host "[GuaxiFonts] font install finished. installed=$installedCount reRegistered=$repairedCount skipped=$skippedCount failed=$failedCount migratedEntries=$($cleanupResult.RemovedEntries) migratedFiles=$($cleanupResult.RemovedFiles) families=$($seenFamilies.Count) unavailableFamilies=$($unavailableFamilies.Count)"
if ($unavailableFamilies.Count -gt 0) {
  exit 1
}
exit 0
