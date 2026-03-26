!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Extracting utils.zip to $INSTDIR\\utils"
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "try { $zip = Join-Path ''$INSTDIR'' ''utils\\utils.zip''; $dst = Join-Path ''$INSTDIR'' ''utils''; if (-not (Test-Path -LiteralPath $zip)) { throw ''utils.zip not found'' }; Expand-Archive -LiteralPath $zip -DestinationPath $dst -Force; Remove-Item -LiteralPath $zip -Force; Set-Content -LiteralPath (Join-Path $dst ''.guaxi_utils_extract_complete'') -Value ''ok'' -Encoding ascii } catch { Write-Error $_; exit 1 }"'
  Pop $0
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压 utils.zip 失败，安装已中止。请检查安装目录权限后重试。"
    Abort
  done:
!macroend
