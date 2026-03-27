!macro NSIS_HOOK_POSTINSTALL
  ; The large utils runtime is distributed as external tar.zst bundle parts
  ; next to the installer. After the main app files are copied, invoke the
  ; dedicated Rust helper to expand those bundles into $INSTDIR\utils.
  DetailPrint "Extracting external utils bundle parts to $INSTDIR\\utils"
  nsExec::ExecToLog '"$EXEDIR\extract_utils_bundles.exe" --bundle-root "$EXEDIR\utils_bundles" --destination "$INSTDIR\utils"'
  Pop $0
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 utils 资源包失败，安装已中止。请确认安装器同级目录下存在 utils_bundles 文件夹并完整重试。"
    Abort
  done:
!macroend
