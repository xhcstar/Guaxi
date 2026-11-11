!macro NSIS_HOOK_POSTINSTALL
  ; The large utils runtime is distributed as external tar.zst bundle parts
  ; next to the installer. After the main app files are copied, invoke the
  ; dedicated Rust helper to expand those bundles into $INSTDIR\utils.
  ; 这一句是安装时唯一能看到的进度提示：解包几 GB 数据期间进度条不会动，
  ; 提前说明「要等一会儿」，避免用户以为安装卡死。详细的分包日志在安装日志里
  ; （extract_utils_bundles 输出的 [utils.bundle] 行由 ExecToLog 收进日志）。
  DetailPrint "Extracting external utils bundle parts to $INSTDIR\utils (several GB, may take a few minutes)"
  nsExec::ExecToLog '"$EXEDIR\extract_utils_bundles.exe" --bundle-root "$EXEDIR\utils_bundles" --destination "$INSTDIR\utils" --state-file "$INSTDIR\.guaxi-utils-bundles.state.json"'
  Pop $0
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 utils 资源包失败，安装已中止。请确认安装器同级目录下存在完整的 utils_bundles 文件夹（含 bundle-manifest.json）后重试。"
    Abort
  done:

  ; The CUDA runtime is far too large for the installer's data block (NSIS
  ; caps it around 2 GB), so the GPU build publishes it as external tar.zst
  ; bundles instead. It must land in $INSTDIR itself, next to guaxi.exe, for
  ; the Windows loader and onnxruntime to find the provider DLLs. CPU builds
  ; ship no such folder and skip this step.
  IfFileExists "$EXEDIR\cuda_runtime_bundles\*.tar.zst" 0 cuda_done
    DetailPrint "Extracting external CUDA runtime parts to $INSTDIR (large, may take a few minutes)"
    nsExec::ExecToLog '"$EXEDIR\extract_utils_bundles.exe" --bundle-root "$EXEDIR\cuda_runtime_bundles" --destination "$INSTDIR" --state-file "$INSTDIR\.guaxi-cuda-bundles.state.json" --no-marker'
    Pop $2
    StrCmp $2 "0" cuda_done 0
      MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 CUDA 运行库失败，安装已中止。请确认安装器同级目录下存在完整的 cuda_runtime_bundles 文件夹（含 bundle-manifest.json）后重试。"
      Abort
  cuda_done:

  ; Install bundled Guaxi fonts after utils have been extracted.
  ; The PowerShell script performs per-user installation and skips families that
  ; already exist in HKCU/HKLM, so the current-user NSIS installer does not need
  ; administrator privileges.
  IfFileExists "$INSTDIR\utils\fonts\install_guaxi_fonts.ps1" 0 font_done
    DetailPrint "Installing bundled Guaxi fonts from $INSTDIR\\utils\\fonts"
    nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\utils\fonts\install_guaxi_fonts.ps1" -FontDir "$INSTDIR\utils\fonts"'
    Pop $1
    StrCmp $1 "0" font_done 0
      MessageBox MB_ICONEXCLAMATION|MB_OK "安装阶段注册 Guaxi 字体失败。软件仍可继续使用，但部分界面字体或数学字体可能回退为系统默认字体。详情请查看安装日志。"
  font_done:
!macroend
