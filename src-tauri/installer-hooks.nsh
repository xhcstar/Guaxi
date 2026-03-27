!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Extracting external utils bundle parts to $INSTDIR\\utils"
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\scripts\extract_utils_bundles.ps1" -BundleRoot "$EXEDIR\utils_bundles" -Destination "$INSTDIR\utils"'
  Pop $0
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 utils 资源包失败，安装已中止。请确认安装器同级目录下存在 utils_bundles 文件夹并完整重试。"
    Abort
  done:
!macroend
