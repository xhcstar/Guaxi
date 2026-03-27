!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Extracting utils bundle parts to $INSTDIR\\utils"
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "try { $bundleRoot = Join-Path ''$INSTDIR'' ''utils\\__bundles''; $dst = Join-Path ''$INSTDIR'' ''utils''; New-Item -ItemType Directory -Force -Path $dst | Out-Null; if (-not (Test-Path -LiteralPath $bundleRoot)) { throw ''utils bundle directory not found'' }; $bundles = Get-ChildItem -LiteralPath $bundleRoot -Filter *.zip -File | Sort-Object Name; if ($bundles.Count -eq 0) { throw ''utils bundle archive not found'' }; foreach ($bundle in $bundles) { Expand-Archive -LiteralPath $bundle.FullName -DestinationPath $dst -Force }; Remove-Item -LiteralPath $bundleRoot -Recurse -Force; Set-Content -LiteralPath (Join-Path $dst ''.guaxi_utils_extract_complete'') -Value ''ok'' -Encoding ascii } catch { Write-Error $_; exit 1 }"'
  Pop $0
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压 utils 资源包失败，安装已中止。请检查安装目录权限后重试。"
    Abort
  done:
!macroend
