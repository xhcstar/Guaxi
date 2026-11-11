!macro NSIS_HOOK_POSTINSTALL
  ; 解压几 GB 资源期间进度条不动，最容易让用户以为安装卡死。这里做三件事：
  ;   1) 把详情面板强行打开，helper 的进度行（[utils.bundle] …）会实时滚进去；
  ;   2) 每一步都写一行「开始 / 结束 + 退出码」，失败时能定位到是哪一步；
  ;   3) 同一个日志同时落到 $INSTDIR\install.log，安装完或失败后都能回看。
  SetDetailsPrint both

  ; NSIS 自己也在同一个日志里留阶段标记：万一 helper 还没启动就失败（文件缺失、
  ; 被杀软拦截），日志里至少能看到安装器走到哪一步了。
  ;
  ; 日志文件统一成 UTF-8（带 BOM）：helper 写的是 UTF-8，而 NSIS 的 FileWrite 只能
  ; 写本地代码页字节——中文混进去会让整个文件变成「一半 UTF-8 一半 GBK」，用记事本
  ; 打开必然有一半是乱码。所以这里的标记行一律用 ASCII 英文，BOM 用 FileWriteByte
  ; 手工写，两边就能拼成一份合法的 UTF-8 文件。
  FileOpen $9 "$INSTDIR\install.log" w
  FileWriteByte $9 0xEF
  FileWriteByte $9 0xBB
  FileWriteByte $9 0xBF
  FileWrite $9 "[nsis] postinstall: expand external utils bundles$\r$\n"
  FileClose $9

  ; The large utils runtime is distributed as external tar.zst bundle parts
  ; next to the installer. After the main app files are copied, invoke the
  ; dedicated Rust helper to expand those bundles into $INSTDIR\utils.
  DetailPrint "正在解压 utils 资源包到 $INSTDIR\utils（数 GB，可能需要几分钟，请勿关闭窗口）"
  DetailPrint "详细进度见下方日志，安装结束后也会保存到 $INSTDIR\install.log"
  ; --console-encoding ansi：nsExec::ExecToLog 把子进程 stdout 按系统本地代码页
  ; 解码，不给这个开关的话 helper 的 UTF-8 中文会在面板里变成乱码。
  nsExec::ExecToLog '"$EXEDIR\extract_utils_bundles.exe" --console-encoding ansi --bundle-root "$EXEDIR\utils_bundles" --destination "$INSTDIR\utils" --state-file "$INSTDIR\.guaxi-utils-bundles.state.json" --log-file "$INSTDIR\install.log"'
  Pop $0
  DetailPrint "utils 资源解包退出码：$0"
  FileOpen $9 "$INSTDIR\install.log" a
  FileWrite $9 "[nsis] utils extraction exit code: $0$\r$\n"
  FileClose $9
  StrCmp $0 "0" done 0
    MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 utils 资源包失败（退出码 $0），安装已中止。$\r$\n$\r$\n请确认：$\r$\n1. 安装器同级目录存在完整的 utils_bundles 文件夹（含 bundle-manifest.json 与全部 .tar.zst）；$\r$\n2. 目标磁盘空间充足；$\r$\n3. 杀毒软件未拦截写入。$\r$\n$\r$\n详细日志：$INSTDIR\install.log"
    Abort
  done:

  ; The CUDA runtime is far too large for the installer's data block (NSIS
  ; caps it around 2 GB), so the GPU build publishes it as external tar.zst
  ; bundles instead. It must land in $INSTDIR itself, next to guaxi.exe, for
  ; the Windows loader and onnxruntime to find the provider DLLs. CPU builds
  ; ship no such folder and skip this step.
  IfFileExists "$EXEDIR\cuda_runtime_bundles\*.tar.zst" 0 cuda_done
    DetailPrint "正在解压 CUDA 运行库到 $INSTDIR（GPU 版，数百 MB 到数 GB，可能需要几分钟）"
    nsExec::ExecToLog '"$EXEDIR\extract_utils_bundles.exe" --console-encoding ansi --bundle-root "$EXEDIR\cuda_runtime_bundles" --destination "$INSTDIR" --state-file "$INSTDIR\.guaxi-cuda-bundles.state.json" --no-marker --log-file "$INSTDIR\install.log"'
    Pop $2
    DetailPrint "CUDA 运行库解包退出码：$2"
    FileOpen $9 "$INSTDIR\install.log" a
    FileWrite $9 "[nsis] cuda runtime extraction exit code: $2$\r$\n"
    FileClose $9
    StrCmp $2 "0" cuda_done 0
      MessageBox MB_ICONSTOP|MB_OK "安装阶段解压外部 CUDA 运行库失败（退出码 $2），安装已中止。$\r$\n$\r$\n请确认安装器同级目录存在完整的 cuda_runtime_bundles 文件夹（含 bundle-manifest.json 与全部 .tar.zst），且目标磁盘空间充足。$\r$\n$\r$\n详细日志：$INSTDIR\install.log"
      Abort
  cuda_done:

  ; Install bundled Guaxi fonts after utils have been extracted.
  ; The PowerShell script performs per-user installation and skips families that
  ; already exist in HKCU/HKLM, so the current-user NSIS installer does not need
  ; administrator privileges.
  IfFileExists "$INSTDIR\utils\fonts\install_guaxi_fonts.ps1" 0 font_done
    DetailPrint "Installing bundled Guaxi fonts from $INSTDIR\utils\fonts"
    nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\utils\fonts\install_guaxi_fonts.ps1" -FontDir "$INSTDIR\utils\fonts"'
    Pop $1
    DetailPrint "字体注册退出码：$1"
    StrCmp $1 "0" font_done 0
      MessageBox MB_ICONEXCLAMATION|MB_OK "安装阶段注册 Guaxi 字体失败。软件仍可继续使用，但部分界面字体或数学字体可能回退为系统默认字体。详情请查看安装日志。"
  font_done:

  DetailPrint "资源解压完成"
  FileOpen $9 "$INSTDIR\install.log" a
  FileWrite $9 "[nsis] postinstall done$\r$\n"
  FileClose $9
!macroend
