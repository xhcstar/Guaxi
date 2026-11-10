# Guaxi 桌面端构建与资源打包说明

> 一句话：**开发时直接读源码树 `src-tauri/utils`；发布时把 `utils` 拆成外部的 `tar.zst` 分包，随安装器一起分发，安装阶段再由专用 helper 解包进安装目录。**

本文最后一次与代码核对：2026-11-10。改动构建链后请回来同步这一份，别再往 `package.json` / `tauri.conf.json` 里写注释（JSON 不支持注释）。

---

## 0. 一分钟版本

| 场景 | 命令 |
|---|---|
| 日常开发 | `npm run tauri -- dev` |
| 正式发布（`utils` 有变化） | `npm run tauri -- build` |
| 正式发布（`utils` 没变，只改了前端/Rust） | `npm run tauri:build:reuse` |
| 只重新生成 `utils` 分包（不构建安装器） | `npm run prepare:tauri-utils` |
| 发布 GPU 版 | `npm run sherpa:fetch:cuda`，再 `npm run tauri -- build -- --no-default-features --features gpu` |
| 分包机制冒烟测试 | `cargo run --manifest-path src-tauri/Cargo.toml --bin verify_utils_bundle_roundtrip` |

Windows 发布时，下面这些必须放在**同一个目录**一起分发：

- `guaxi_*.exe`（NSIS 安装器）
- `utils_bundles/`（若干 `*.tar.zst` + `bundle-manifest.json`）
- `extract_utils_bundles.exe`
- `README_utils_bundles.txt`（构建自动生成，方便发版时核对内容）
- GPU 版额外需要 `cuda_runtime_bundles/`（同样含 `bundle-manifest.json`）

少任何一个，安装到一半就会弹框中止。

---

## 1. 这套方案要解决的问题

1. **开发要快。** 每次 `dev` 都复制/解压几 GB 的 `utils` 无法接受，所以 `debug` 下直接复用源码目录。
2. **安装包有硬上限。** 把多 GB 资源和主程序一起塞进 NSIS，会触发 `makensis` 的 `error mmapping datablock`（数据块约 2GB 上限）或内部编译器越界。所以大资源一律走**安装器同级目录的外部 tar.zst 分包**。
3. **运行时路径必须稳定。** 所有业务代码永远按 `utils/...` 访问资源，安装后解包结果不能出现 `utils/utils/...` 这种错层。
4. **构建期不能联网。** 构建机往往只有注册表里的系统代理、没有 `*_PROXY` 环境变量，由 Cargo 自行下载原生库会直接卡死。所有外部依赖改成「脚本幂等预取 + 构建期零联网」。
5. **增量可复用、可恢复。** 几百 MB 的重压缩不能每次都做；安装中断后再次启动也不能让用户面对半个 `utils`。

---

## 2. 关键文件地图

### 2.1 入口与编排（前端侧）

| 文件 | 职责 |
|---|---|
| `package.json` | npm 脚本入口（见第 3 章） |
| `tools/run_tauri_with_utils.mjs` | **唯一的构建编排层**：预取依赖 → 准备静态资源 → 生成分包 → 编译 helper → 调 `tauri` → 自检 → 发布外部文件 |
| `tools/fetch_codex_sidecar.mjs` | 取 `utils/codex/codex.exe`（失败不阻断） |
| `tools/fetch_sherpa_onnx_libs.mjs` | 取语音原生库、模型、CUDA 运行时（失败**阻断**，见第 9 章） |
| `tools/copy_excalidraw_assets.mjs` | 把 Excalidraw / Vditor 的静态资源复制进 `public/` |
| `tools/build_chart_preview_bundle.mjs` | 生成 `src-tauri/assets/vendor/guaxi-chart-preview.js`（被 Rust `include_str!` 打进二进制） |
| `tools/prepare_rsshub_env.mjs` | 把现代 RSSHub clone + build 进 `utils/rsshub`（失败不阻断） |

### 2.2 Rust 侧

| 文件 | 职责 |
|---|---|
| `src-tauri/build.rs` | release 编译前的**兜底**分包；debug 下只打日志跳过；另负责主线程栈大小与预览资源指纹 |
| `src-tauri/utils_zip_support.rs` | **核心实现**：扫描、拆分、压缩、清单、增量解包、校验、跳过规则全在这里 |
| `src-tauri/src/bin/prepare_utils_zip.rs` | 显式生成 `utils` 分包的 CLI |
| `src-tauri/src/bin/prepare_cuda_runtime_bundle.rs` | 把 CUDA 运行时 DLL 打包成外部 payload 的 CLI |
| `src-tauri/src/bin/extract_utils_bundles.rs` | 安装阶段解包用的 helper（也是给 CUDA payload 用的同一个程序） |
| `src-tauri/src/bin/verify_cuda_runtime.rs` | GPU 版打包后自检 CUDA 运行库能否加载 |
| `src-tauri/src/bin/verify_utils_bundle_roundtrip.rs` | 分包/解包/跳过规则的冒烟测试 |
| `src-tauri/src/guaxiservices/bundled_resources.rs` | 运行时 `utils/...` 路径解析、恢复、按需解包 |
| `src-tauri/.cargo/config.toml` | 把 `SHERPA_ONNX_ARCHIVE_DIR` 指向预取归档，构建期零联网 |
| `src-tauri/.taurignore` | 让 `tauri dev` 忽略 `utils/`、`logs/`、`target/` 等运行期写入，避免无意义的 Rust 全量重建 |

### 2.3 配置与安装

| 文件 | 职责 |
|---|---|
| `src-tauri/tauri.conf.json` | `beforeDevCommand` / `devUrl` / `beforeBuildCommand` / `frontendDist` |
| `src-tauri/tauri.windows.conf.json` | Windows：挂 NSIS 钩子 `installer-hooks.nsh`；**不**把 `utils` 打进安装包，只带 `scripts/`、`libs/`、`agent_server/` |
| `src-tauri/tauri.linux.conf.json` | Linux：`appimage/deb/rpm`，仍把 `utils/__bundles/` 作为 resources 打进包 |
| `src-tauri/installer-hooks.nsh` | `NSIS_HOOK_POSTINSTALL`：解包 `utils` → 解包 CUDA → 安装字体 |

> 范围说明：Windows（NSIS）是当前唯一完整验证并实际发布的主线。Linux 配置仍保留原来的做法（把 `utils/__bundles` 当 resources 打进包），安装期解包与运行时路径解析没有针对 deb/rpm/AppImage 的布局单独适配，改这条线之前请先验证。

---

## 3. npm 脚本速查

### 3.1 开发

| 脚本 | 说明 |
|---|---|
| `npm run dev` | 只起 Vite |
| `npm run tauri -- dev` | Tauri 开发模式（走编排层） |

### 3.2 构建

| 脚本 | 说明 |
|---|---|
| `npm run build` | 构建前端产物到 `dist/`（末尾带 `check:reader-chunks`） |
| `npm run tauri -- build` | 完整 release 构建（推荐） |
| `npm run tauri:build` | 等价写法（`build`） |
| `npm run tauri:build:reuse` | 等价于 `build --reuse-utils-zip`：**信任已有清单、不重扫 `utils`** |
| `npm run tauri -- bundle` / `npm run tauri:bundle` | 只做打包（不重新编译） |
| `npm run tauri:bundle:reuse` | 上述 + 复用分包 |
| `npm run prepare:tauri-assets` | 单独复制 Excalidraw/Vditor 资源 |
| `npm run prepare:tauri-utils` | 单独生成分包（等价 `cargo run --bin prepare_utils_zip`） |

### 3.3 依赖预取

| 脚本 | 说明 |
|---|---|
| `npm run codex:fetch` / `:force` | 取 Codex sidecar |
| `npm run sherpa:fetch` / `:force` | 取语音原生库与模型（幂等） |
| `npm run sherpa:fetch:cuda` | 额外装配 GPU 原生库 + CUDA 运行时（约 2GB） |
| `npm run sherpa:verify:cuda` | 单独跑 CUDA 自检（脚本只带了 `--`，需自行追加 `--dir <release 产物目录>`） |
| `npm run rsshub:prepare` / `:force` | 构建 RSSHub 运行环境 |

### 3.4 关于那些「历史命名」

以下名字里都带 `zip`，但**产物早已不是 zip**，现在一律是 `tar.zst` 分包：

- 参数 `--reuse-utils-zip`（内部转成 `--reuse-existing`）
- 环境变量 `GUAXI_REUSE_UTILS_ZIP`
- 二进制 `prepare_utils_zip`、npm 脚本 `prepare:tauri-utils`

保留它们只是为了不打断已有的本地命令和 CI。**语义以本文为准**：`reuse` = 复用已生成的 `*.tar.zst` 分包。

---

## 4. 两种模式的总览

```text
dev
  npm run tauri -- dev
    └─ 编排层：fetch codex（容错）→ fetch sherpa（必成功）
       └─ tauri dev → Vite(:1420) + cargo(debug)
            └─ build.rs：PROFILE=debug，跳过分包
       └─ 运行时：ensure_relative_dir("utils/...") → 直接命中 src-tauri/utils

release
  npm run tauri -- build
    ├─ S0 预取：codex（容错）、sherpa 库与模型（必成功）
    ├─ S0'（仅 GPU）CUDA 原生库校验 → prepare_cuda_runtime_bundle
    ├─ S1 前端静态资源：excalidraw/vditor、chart-preview、rsshub（容错）
    ├─ S2 utils 分包：prepare_utils_zip → src-tauri/utils/__bundles/*.tar.zst + bundle-manifest.json
    ├─ S3（仅 Windows）编译 extract_utils_bundles.exe
    ├─ S4 tauri build：beforeBuildCommand → npm run build → cargo release（build.rs 兜底但信任清单）
    ├─ S5（仅 GPU）verify_cuda_runtime 自检
    └─ S6 发布外部文件到 target/release/bundle/nsis/
```

---

## 5. dev 模式

### 5.1 调用链

1. `npm run tauri -- dev` → `tools/run_tauri_with_utils.mjs`。
2. 无论子命令是什么，**先跑两步预取**（都幂等）：
   - `node ./tools/fetch_codex_sidecar.mjs`（不校验退出码：拿不到就回退内置 Python Agent，不阻断开发）
   - `node ./tools/fetch_sherpa_onnx_libs.mjs`（**校验退出码**：静态库缺失会让 cargo 直接失败）
3. `dev` 不属于 `build/bundle`，所以**跳过**所有 release 专属步骤：静态资源复制、分包、helper 编译。
4. 调 `tauri dev`；Tauri 读 `tauri.conf.json`：`beforeDevCommand = npm run dev`、`devUrl = http://localhost:1420`。
5. Cargo 编译 `guaxi` 前先跑 `build.rs` → `PROFILE=debug` → 打印 `[utils.bundle] skipped in debug profile`，不生成分包。

### 5.2 dev 下 `utils/...` 是怎么解析到的

统一入口是 `bundled_resources::ensure_relative_dir()`（`src-tauri/src/guaxiservices/bundled_resources.rs`），解析顺序：

1. 目标目录 `exe_dir/utils` 若已有内容 → 直接用。
2. 否则（debug 下）从 exe 往上找同时含 `Cargo.toml` 与 `utils/` 的祖先目录 → 命中 `src-tauri/utils`。
3. 都没有就退回 `exe_dir/utils`（空目录），由调用方处理缺失。

另有一条**按文件**的兜底：debug 下若解析到的具体文件不存在（典型是 `pandoc.exe`、`tectonic` 这类只随源码走的工具），会改用源码树里的同名文件。这就是为什么 `target/debug/utils` 里即使有 `python_runtime` 等运行期目录，也不会把工具链遮住。

### 5.3 为什么 dev 不复制 `utils`

几 GB 的复制/解压会让每次启动都慢；而且本地改了 `src-tauri/utils` 的内容能立刻被 dev 用上，省去一轮同步。

代价（必须守住）：

- 所有访问 `utils/...` 的 Rust 代码都要走 `ensure_relative_dir()`；
- **不要**自己拼 `exe_dir().join("utils")`，那样 debug 下会指到 `target/debug/utils` 而失败。

### 5.4 `.taurignore` 的意义

`tauri dev` 会监听整个 `src-tauri` 工作区。`utils/`、`logs/`、`target/`、`*.log`、`__pycache__` 这些每次运行都会被写入，若不在忽略表里，就会不停触发「文件变化 → 重新编译 Rust」。新增运行期写入目录时，记得同步 `src-tauri/.taurignore`。

---

## 6. release 模式

### 6.1 阶段明细

| 阶段 | 动作 | 失败行为 |
|---|---|---|
| S0 | `fetch_codex_sidecar.mjs` | **忽略**，仅在日志里告警 |
| S0 | `fetch_sherpa_onnx_libs.mjs` | **中止**（缺静态库 cargo 必失败） |
| S0' | GPU 构建：检查 `.sherpa-onnx-libs/1.13.8/cuda-lib` 是否存在 | 不存在直接退出，提示先跑 `npm run sherpa:fetch:cuda` |
| S0' | GPU 构建：`cargo run --bin prepare_cuda_runtime_bundle -- --source-dir <cuda-lib> --bundle-dir src-tauri/target/cuda-runtime-bundles` | 中止 |
| S1 | `copy_excalidraw_assets.mjs` | 中止 |
| S1 | `build_chart_preview_bundle.mjs` | 中止（Rust 侧 `include_str!` 依赖该产物） |
| S1 | `prepare_rsshub_env.mjs` | **忽略**，缺 RSSHub 只降级聚合功能 |
| S2 | `cargo run --bin prepare_utils_zip`（带 `--reuse-utils-zip` 时追加 `-- --reuse-existing`） | 中止 |
| S3 | 仅 Windows：`cargo build --release --bin extract_utils_bundles` | 中止 |
| S4 | `tauri build/bundle`（含 `beforeBuildCommand = npm run build`） | 中止 |
| S5 | 仅 GPU：`verify_cuda_runtime --dir target/release` | 退出码 2 中止；其它非 0 只告警 |
| S6 | 仅 Windows：把分包、helper、README 发布到 `target/release/bundle/nsis/` | 中止（找不到分包会直接报错） |

要点补充：

- **S2 必须在 S4 之前**：`prepare_utils_zip` 生成的清单要先进位，S4 里 `build.rs` 才能「信任清单」而不是重扫十几万文件。
- **S3 为什么要单独设环境变量**：helper 也是本包的一个 bin，release 编译会再触发一次 `build.rs`。所以 S3 显式传 `GUAXI_SKIP_BUNDLE_PREPARE=1`（复用模式下再加 `GUAXI_REUSE_UTILS_ZIP=1`），否则会出现「主流程已备好分包，helper 又重打一遍」。
- **S4 传给 `tauri` 的环境**：`GUAXI_REUSE_UTILS_ZIP=1`（让 `build.rs` 信任刚生成的清单）；GPU 构建再加 `SHERPA_ONNX_LIB_DIR`（只注入这一层，不能挂到 `process.env`，否则前面以默认 `static` 特性跑的辅助构建会找不到静态库）。
- **Windows 内存优化**：`releaseCargoEnv()` 在 Windows 上固定 `CARGO_BUILD_JOBS=1`、`CARGO_INCREMENTAL=0`。这个 crate 的 release 链接/codegen 峰值很高，串行化 + 关增量能显著降低 rustc/LLVM OOM 概率。

### 6.2 环境变量一览

| 变量 | 设置者 | 作用 |
|---|---|---|
| `GUAXI_REUSE_UTILS_ZIP=1` | 编排层 | `build.rs` 信任已有 `bundle-manifest.json`，不重扫 `utils` |
| `GUAXI_SKIP_BUNDLE_PREPARE=1` | 编排层（helper 编译） | `build.rs` 完全跳过分包准备 |
| `SHERPA_ONNX_ARCHIVE_DIR` | `.cargo/config.toml` | 静态库归档位置，构建期零联网 |
| `SHERPA_ONNX_LIB_DIR` | 编排层（仅 GPU 的最终调用） | 动态库（CUDA）目录 |
| `CARGO_BUILD_JOBS` / `CARGO_INCREMENTAL` | 编排层（仅 Windows release） | 降低 release 构建内存峰值 |

### 6.3 构建产物

```text
src-tauri/utils/__bundles/
    <bundle>.tar.zst ...
    bundle-manifest.json

src-tauri/target/release/
    guaxi.exe
    extract_utils_bundles.exe
    (GPU) onnxruntime*.dll、cudart64_*.dll ... 由 sherpa-onnx-sys 的 build.rs 拷来

src-tauri/target/release/bundle/nsis/
    guaxi_<ver>_x64-setup.exe
    extract_utils_bundles.exe
    README_utils_bundles.txt
    utils_bundles/            ← 含 bundle-manifest.json
    cuda_runtime_bundles/     ← 仅 GPU；CPU 构建会主动删除上一次遗留的该目录
```

`README_utils_bundles.txt` 由编排层生成，列出每个分包的文件名与字节数，发版时用来核对「带出去的到底是不是这一版」。

---

## 7. `utils` 分包机制

### 7.1 为什么是 `tar.zst`

- `tar` 把小文件组织成连续流，减少碎文件开销；
- `zstd` 压缩/解压都快，支持多线程；
- 同一份 Rust 实现（`utils_zip_support.rs`）同时被构建期和安装期复用，行为一致；
- 保留原始相对路径，解包后结构天然对齐 `utils/...`。

### 7.2 拆分规则（重要，和旧文档不同）

不是「一个顶层目录一个包」那么简单，实际是**按体积递归下拆**：

1. 扫描 `src-tauri/utils` 顶层条目（`should_skip` 过滤后）。
2. 每个条目先按整体算体积；若**同时满足**：
   - 深度 < `MAX_SPLIT_DEPTH`（6）
   - 体积 > `SPLIT_THRESHOLD_BYTES`（256MB）
   - 子项数 ≤ `MAX_SPLIT_CHILDREN`（64）

   则**继续下拆到它的子项**，递归判断（深度 +1）。
3. 顶层单个文件天然自成一包。

这样做的目的：换一个 800MB 的模型文件不必重压它旁边的其他文件；而 `node_modules` 那种成千上万子项的大目录不会被炸成上千个小包（子项超限就整体一包）。

实际效果举例（同一目录下会混着两种命名）：

- `pandoc.exe` → `pandoc.exe.tar.zst`
- `python_runtime/`（小） → `python_runtime.tar.zst`
- `Models/asr/sense-voice/`（大） → 被拆成 `Models%2Fasr%2Fsense-voice%2Fmodel.onnx.tar.zst` 等若干包

常量都在 `utils_zip_support.rs` 顶部，调完记得清一次 `__bundles` 重打（见 10.8）。

### 7.3 分包命名

- 名字 = 源相对路径（`/` 分隔）；
- 转义：`%` → `%25`，`/` → `%2F`（例如 `Models%2Fasr%2Fsense-voice%2Fmodel.onnx.tar.zst`）；
- 转义后超过 180 字符 → 退化为 `bundle-<blake3 前 16 位>.tar.zst`；
- 同一批里名字冲突会直接报错（`bundle name collision`）。

### 7.4 `bundle-manifest.json`（schema = 2）

这是**跨构建/安装两端的契约文件**，缺了它安装器会拒绝工作。每个条目记录：

| 字段 | 含义 |
|---|---|
| `name` | 分包文件名 |
| `source` | 源相对路径（`utils/` 下，`/` 分隔） |
| `source_kind` | `file` / `dir` |
| `digest` | 源指纹：路径 + 大小 + mtime（不读文件内容，所以「只改 mtime」也会触发重打） |
| `archive_bytes` / `archive_blake3` | 产物大小与内容哈希 |
| `files[]` | 包内每个文件的 `path` / `size` / `mtime_ns` |
| `empty_dirs[]` | 空目录（必须显式记录，否则解包后丢失） |

用途：

- 构建端：判断「这个包能不能直接复用」；
- 安装端：校验归档完整性（`archive_blake3`）、判断目标文件是否已就位（`size` + `mtime`）；
- 发布端：`README_utils_bundles.txt` 与排查「包和安装包是否同源」。

### 7.5 三级复用，别搞混

| 层级 | 触发条件 | 省掉什么 |
|---|---|---|
| ① 整树跳过 | `--reuse-existing` + 清单完整（schema 对、所有包存在且大小一致） | **连扫描都省**（十几万文件的 stat） |
| ② 单包复用 | 无该开关时，逐包比对 `digest` 且归档大小一致 | 只重压变化的包 |
| ③ 文件级跳过 | 安装端：安装状态文件里的 key 与清单一致，且目标文件 size/mtime 匹配 | 不重写已就位的文件 |

**注意 ① 的副作用**：`npm run tauri:build:reuse` 不会重新扫描 `utils`。如果你刚往 `utils` 里加了文件却又用 reuse 命令，新文件**不会**进包（日志里是 `[utils.bundle] reusing existing`）。这种时候要么跑一次不带 reuse 的 `npm run tauri -- build`，要么先 `npm run prepare:tauri-utils`。

### 7.6 什么不进包

排除规则集中在 `utils_zip_support.rs`：`SKIPPED_DIRECTORIES` + `should_skip()`。

| 排除项 | 原因 |
|---|---|
| `MarkdownConvertor` | 可选组件，随安装目录独立分发，缺失时运行时回退 marker |
| `MinerU`、`Models/mineru_models` | 可选强引擎（大体积 venv + 模型），同上 |
| `Models/surya_models` | 未纳管的大模型目录 |
| `sherpa-onnx`、`whisper` | 语音已改为进程内库调用，外部可执行文件已退役 |
| `__bundles` | 产物目录自身 |
| `*.tar.bz2` | 下载归档，只是中间产物，留着会白撑几百 MB |
| 旧版 `utils.zip` 等 | 历史遗留产物 |
| `.__extracting_*`、`.guaxi_*_extract_complete` | 解包中间态与完成标记 |

`should_skip()` 有单测钉住（`skips_download_archives_and_retired_speech_binaries`），改规则请同步改测试。

---

## 8. 安装与运行时

### 8.1 NSIS 钩子顺序（`installer-hooks.nsh`）

`NSIS_HOOK_POSTINSTALL` 在主程序文件复制完之后依次执行：

1. **`utils`**：`extract_utils_bundles.exe --bundle-root "$EXEDIR\utils_bundles" --destination "$INSTDIR\utils" --state-file "$INSTDIR\.guaxi-utils-bundles.state.json"`，失败弹框并 `Abort`。
2. **CUDA**（仅当 `$EXEDIR\cuda_runtime_bundles\*.tar.zst` 存在）：同一 helper，`--destination "$INSTDIR"`（DLL 必须和 `guaxi.exe` 同目录）、`--state-file "$INSTDIR\.guaxi-cuda-bundles.state.json"`、`--no-marker`（不往安装根写 marker）。
3. **字体**：若 `$INSTDIR\utils\fonts\install_guaxi_fonts.ps1` 存在，按用户安装字体；失败只告警，不中止。

`$EXEDIR` = 安装器（即"发布目录"）位置，`$INSTDIR` = 应用安装目录。

### 8.2 helper 参数与语义

| 参数 | 说明 |
|---|---|
| `--bundle-root` | 分包目录（必填） |
| `--destination` | 解包目标（必填） |
| `--state-file` | 增量安装状态；**不传**则退化为全量解包（兼容旧包） |
| `--no-marker` | 不写完成标记（用于落在安装根的 payload，如 CUDA） |

退出码约定：`0` 成功；`1` 解包/校验失败；`2` 缺必填参数。任一失败都不要放过——半套资源比装不上更难排查。

安装前会对要写入的每个归档做**全量预检**（大小 + `archive_blake3`）。这样"混版本"的发布目录会在动手之前就报错，而不是装出一半新一半旧。

### 8.3 增量安装

- 状态文件记录 `分包名 → archive_blake3`；
- 某包在状态里且目标文件 `size`+`mtime` 都对得上 → 直接跳过（日志 `skippedBundles=`）；
- 每解完一包就落一次状态，中途失败下次能接着来；
- 安装完成后清理状态里已不存在的旧包条目。

**重要限制**：解包只写不删。新版本里删掉的资源，不会从已安装用户机器上消失。要真正回收磁盘，得靠重装或另做清理逻辑（见 10.3）。

### 8.4 运行时恢复

`bundled_resources.rs` 在 release 下的处理：

1. 若 `exe_dir/utils/__bundles` 有分包（只有把分包作为 resources 一起打的平台才会出现，当前是 Linux 配置），且标记 `exe_dir/.guaxi_utils_extract_complete` 的时间**早于**最新分包 mtime → 认为需要重解。
2. 解包先落到 `exe_dir/utils.__extracting_<pid>/__payload`，成功后写 `.__extracting_<pid>/.guaxi_extract_complete`。
3. 再把 payload 内容逐个移入 `utils/`，最后写 `exe_dir/.guaxi_utils_extract_complete`，删临时目录。
4. 下次启动若发现残留的 `utils.__extracting_*`：若它带完成标记 → 视为"只差搬运"，继续搬完；否则直接清掉重来。

标记文件放在 `utils` **同级**（不是里面），所以用户看到的 `utils` 只有真资源。旧的"标记在 `utils` 内部"的路径在启动时会被主动删除。

### 8.5 为什么用独立 Rust helper

试过 NSIS 内联 PowerShell / `Expand-Archive`，问题是：引号转义脆弱、解压慢、大资源不稳定。独立 helper 的好处：

- 与构建期共用同一套 Rust 逻辑（同样的 tar.zst、同样的校验、同样的跳过规则）；
- 不受 shell 层引号与编码影响；
- 可单独跑、可单独测（`verify_utils_bundle_roundtrip`）。

---

## 9. GPU（CUDA）构建

### 9.1 两个互斥 feature

```toml
default = ["sherpa-onnx/static"]   # CPU：静态链接，自包含，无需分发 DLL
gpu     = ["sherpa-onnx/shared"]   # GPU：动态链接官方 CUDA 运行库
```

`static` 与 `shared` 同时打开时 `sherpa-onnx-sys` 的 build.rs 直接报错，所以 **GPU 构建必须带 `--no-default-features`**：

```bash
npm run tauri -- dev   -- --no-default-features --features gpu
npm run tauri -- build -- --no-default-features --features gpu
```

> `package.json` 里的 `tauri:gpu` 是 `dev --features gpu`，没有关默认特性。要 GPU 请用上面两条显式写法。

`SHERPA_ONNX_LIB_DIR` 不用手设：编排层识别到 `--features gpu` 会自动指向装配好的 `cuda-lib` 目录；目录不存在则直接报错并提示先跑 `npm run sherpa:fetch:cuda`。

### 9.2 依赖预取（`fetch_sherpa_onnx_libs.mjs`）

脚本做四件事：

1. 静态库归档 → `src-tauri/.sherpa-onnx-libs/1.13.8/`（归档名随平台/架构而异，必须与 `sherpa-onnx-sys` build.rs 期望的逐字一致）；
2. 模型 → `src-tauri/utils/Models/`：Kokoro TTS、SenseVoice ASR、whisper ONNX、Silero VAD、CT-Transformer 标点；
3. 下载走 `curl`（代理解析顺序：`*_PROXY` 环境变量 → Windows 注册表系统代理 → 直连），支持 `.part` 断点续传与低速看门狗；
4. `--cuda` 时额外装配 GPU 版原生库与 CUDA 运行时 DLL。

**为什么不让 Cargo 自己下**：`sherpa-onnx-sys` 默认会去 GitHub 拉预编译库，纯注册表代理环境下会卡死。预取后由 `.cargo/config.toml` 的 `SHERPA_ONNX_ARCHIVE_DIR` 指过去，构建期零联网。

### 9.3 为什么要自己造 `onnxruntime.lib`

官方 CUDA 包 `sherpa-onnx-v1.13.8-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-win-x64-cuda.tar.bz2` 的 `lib/` 里只有：

- `sherpa-onnx-c-api.lib` / `-cxx-api.lib`（DLL 导入库）
- `onnxruntime.dll`、`onnxruntime_providers_cuda.dll` 等（运行时 DLL）

而 shared 模式会发出两条链接指令：

```text
cargo:rustc-link-lib=dylib=sherpa-onnx-c-api
cargo:rustc-link-lib=dylib=onnxruntime
```

第二条要求存在 `onnxruntime.lib`（**导入库**），CUDA 包里没有。静态包里的同名文件是**静态库**，语义不同，拷过来没用。

解法：`dumpbin /exports onnxruntime.dll` 取导出表 → 生成 `.def` → `lib /def:... /machine:x64 /out:onnxruntime.lib`。脚本用 `vswhere` 定位 MSVC；找不到会明确告警并跳过 GPU 准备（不会产出半个可用目录）。

装配后的 `cuda-lib/` 同时放导入库与 DLL——DLL 放在一起是有意的：shared 模式下 build.rs 会把该目录的 DLL 自动拷到 exe 旁，dev 与 release 都覆盖。

### 9.4 CUDA 运行时为什么要外置

GPU 版需要 `onnxruntime*.dll` 与 `cudart/cublas/cublasLt/cudnn/cufft/nvrtc` 等 DLL，合计 2GB 以上。塞进 NSIS 会顶到数据块上限，报 `error mmapping datablock`。

所以复用与 `utils` 相同的机制：`prepare_cuda_runtime_bundle` 只挑 `*.dll`（`include_extension = "dll"`）压成 `cuda_runtime_bundles/*.tar.zst`，安装时由同一个 helper 解到 `$INSTDIR`（`guaxi.exe` 旁边，Windows 加载器与 onnxruntime 都在那里找 DLL）。

CPU 构建会把上一次遗留的 `cuda_runtime_bundles/` 主动删掉，避免"CPU 包里混进 CUDA DLL"。

### 9.5 打包后自检

`verify_cuda_runtime --dir <release 目录>`：

- 退出码 **2**：目录里缺少该有的文件 → **打包缺陷，中止构建**，修复方式 `npm run sherpa:fetch:cuda`；
- 退出码 **3**：文件齐但本机加载不了（没插 N 卡、驱动太旧）→ 只告警，构建继续；
- 其它非 0：告警。

### 9.6 运行时为什么要"先探测再请求"

`provider=cuda` 有个坑：CUDA 组件缺失时 onnxruntime 抛的 C++ 异常会穿透 FFI，Rust 捕不住，直接 abort 进程（`Rust cannot catch foreign exceptions`）。所以"先试 cuda、失败回退 cpu"根本来不及执行回退。

现在的做法：`speech/config.rs` 的 `cuda_available()` 在构造引擎**之前**检查 `nvcuda.dll` 与 `onnxruntime_providers_cuda.dll` 是否都在，不在就降级 cpu 并打 warn。

---

## 10. 变更指南：增删资源、改结构

这一章是本文的重点。先记四条铁律：

1. 运行时访问资源**只能**走 `ensure_relative_dir("utils/...")`，别自己拼路径。
2. 大文件/大目录**不要**加进 `tauri.*.conf.json` 的 `bundle.resources`（会顶爆 NSIS）。
3. 改完 `utils` 必须**重新生成分包**（`reuse` 命令不会重扫）。
4. 动 `utils_zip_support.rs` 的规则/格式，先想安装端与你已发出的旧包是否兼容。

### 10.1 决策表

| 你想做的事 | 要改的地方 | 需要重新出安装包吗 |
|---|---|---|
| 在已有目录里加/换一个文件 | 只放文件 | 需要（该目录的包会重打） |
| 新增一个顶层目录或文件 | 只放文件；按需加 `.gitignore` / fetch 脚本 | 需要 |
| 让某目录**不进** release 包 | `SKIPPED_DIRECTORIES` + 单测 | 需要 |
| 删除一个资源 | 删文件 | 需要；但**已安装用户不会被清理** |
| 重命名/移动顶层目录 | 改目录 + 改所有 `ensure_relative_dir("utils/旧名/...")` 调用 | 需要 |
| 调整分包粒度 | `SPLIT_*` 常量 | 需要（建议清空 `__bundles`） |
| 改归档格式/清单字段 | `BUNDLE_FORMAT_VERSION` + 两端同步 | 需要，且旧 helper 不认新清单 |
| 新增"必须落在安装根"的大 payload | 仿 CUDA 那一套（见 10.6） | 需要 |
| 新增需要联网预取的构建依赖 | 新 `tools/fetch_*.mjs` + 接到编排层正确阶段 | 需要（构建机首次） |
| 前端静态资源（编辑器/查看器） | `public/` 或 `copy_excalidraw_assets.mjs` | 需要 |

### 10.2 新增资源：按情形处理

**情形 A：往已有目录里加文件（最常见）**

直接放进 `src-tauri/utils/<已有目录>/`，然后跑一次不带 reuse 的构建：

```bash
npm run tauri -- build
```

- 该目录若已是一个**独立分包**（体积没超阈值、或子项太多没再下拆）→ 只会重压这一个包，其余命中 `digest` 复用。
- 该目录若**被拆到了子项**（大目录且子项少）→ 只有它所在的下一层单元重压。
- 若目录体积跨过了 256MB 阈值，拆分点会变：原来一整包，之后变成按子项拆；旧包在下一次扫描时被 `prune_stale_bundles` 删掉，属正常现象。

**情形 B：新增一个顶层目录/文件**

1. 放进去（例如 `src-tauri/utils/myrt/`）。
2. **大二进制不要进 git**：确认它已被 `.gitignore` 忽略，改用幂等的 fetch 脚本预取（见 10.7）。
3. 若它是"每次运行都会写"的目录，同步 `src-tauri/.taurignore`，否则 dev 会被反复重启构建。
4. 业务代码一律用 `ensure_relative_dir("utils/myrt/xxx")` 取路径。
5. 跑 `npm run prepare:tauri-utils`，确认日志里的 `bundleCount` 增加、`bundle-manifest.json` 里出现 `"source": "myrt"`。
6. 跑 `npm run tauri -- build` 出包，并**在干净机器上安装验证**该文件确实落在 `$INSTDIR\utils\myrt\...`。

**情形 C：新增体积 ≥ 256MB 的目录**

会自动按子项下拆（≤64 个子项时）。如果子项很多（比如自带 `node_modules`），会保持"整目录一包"，此时：

- 好处：包数不爆炸；
- 代价：改一个文件要重压整包。

要更细的粒度，就把大目录**主动组织成几层**（例如 `myrt/models/a/`、`myrt/models/b/`），或调 `MAX_SPLIT_CHILDREN`（慎调，会连带影响其它目录）。

**情形 D：新增"不放 `utils`、必须落在安装根"的资源**

见 10.6，别塞进 `utils`（`utils` 会被整体解到 `$INSTDIR\utils`，多一层目录就错位）。

**情形 E：新增可选组件（不随主包发）**

不要放进 `utils`。放独立分发目录，运行时按"存在即启用、缺失即回退"处理，并把目录名登记到 `SKIPPED_DIRECTORIES`，防止哪天有人把它拷进 `utils` 又被自动打进包。

### 10.3 删除资源

1. 直接删 `src-tauri/utils/<x>`。
2. 清掉代码里的引用（`ensure_relative_dir("utils/<x>/...")`）。
3. 跑一次 `npm run prepare:tauri-utils`：不再需要的 `*.tar.zst` 会被删除（`prune_stale_bundles`），清单同步重写。
4. 如属"整体退役"（像 `sherpa-onnx`、`whisper` 那样），建议把目录名加进 `SKIPPED_DIRECTORIES`，避免以后误放回来又被静默打包。
5. **重要**：安装端不做删除。旧用户机器上的文件仍在。若体积敏感，必须在发版说明里要求重装，或另外实现清理逻辑（当前没有）。

### 10.4 重命名 / 移动 / 重构目录

这是一次**契约变更**，四件事一起做：

1. 改目录。
2. 改所有访问点的 `ensure_relative_dir(...)` 字符串。
3. 重新生成分包（旧包名会因 `prune_stale_bundles` 消失）。
4. 验证安装：`$INSTDIR\utils\<新名>\...` 存在，且运行时没报"资源缺失"。

注意分包名是对路径转义的（`/` → `%2F`），改名会让对应包"换名字"，对已安装用户来说相当于一批新文件被写入；旧文件不会删（见 10.3）。

### 10.5 精确控制"什么不进包"

改 `src-tauri/utils_zip_support.rs`：

```rust
const SKIPPED_DIRECTORIES: &[&str] = &[
    "MarkdownConvertor",
    "Models/surya_models",
    "MinerU",
    "Models/mineru_models",
    "sherpa-onnx",
    "whisper",
    GENERATED_BUNDLES_DIR,
];
```

- 匹配规则：等于该相对路径，或以 `"<dir>/"` 开头（即整棵子树）；
- 同时同步 `should_skip()` 的单测；
- 改完记得跑一次 `prepare:tauri-utils` 看 `bundleCount` 有没有按预期变化。

### 10.6 新增一类"外置 payload"（照 CUDA 抄）

以现有的 CUDA 为模板，共五处：

1. **打包 CLI**：新建 `src-tauri/src/bin/prepare_<x>.rs`，调 `utils_zip_support::prepare_payload_bundles(&source, &bundle_dir, reuse, Some("dll"))`（`include_extension` 只对顶层文件生效；`None` 表示全收）。
2. **编排层**：在 `tools/run_tauri_with_utils.mjs` 的 `shouldPrepareForTauriCommand` 分支里 `cargo run --bin prepare_<x> -- --source-dir ... --bundle-dir src-tauri/target/<x>-bundles`；路径统一走 `projectRoot` 拼，别写死。
3. **发布**：在 `publishExternalWindowsBundles()` 里 `collectBundleEntries` + `publishBundleDir` 到 `target/release/bundle/nsis/<x>_bundles/`，并把它写进自动生成的 `README_utils_bundles.txt`。
4. **安装**：在 `installer-hooks.nsh` 加一段 `IfFileExists "$EXEDIR\<x>_bundles\*.tar.zst"` → 调同一个 `extract_utils_bundles.exe`，`--destination` 决定落点（安装根用 `$INSTDIR`，并加 `--no-marker`），失败 `Abort`。
5. **清理**：CPU 版要把上一次遗留的目录删掉（照 `rm(windowsCudaRuntimeBundlesDir)` 的写法），避免混发。

### 10.7 新增需要联网预取的构建依赖

模板：`tools/fetch_sherpa_onnx_libs.mjs`。要点：

- **幂等**：已存在且可用就跳过（用文件大小/关键文件做 `ready()` 判断）；
- **断点续传**：`curl -C -` + `.part`，失败不留残包；
- **代理**：`*_PROXY` → Windows 注册表 → 直连；
- **别用绝对盘符调 GNU tar**（Git Bash 会把 `D:\` 当主机名），一律"相对文件名 + cwd"；
- **接线位置决定成败**：
  - 供 Cargo 链接的原生库 → 必须早于**任何** cargo 构建；
  - 供打包的资源 → 必须早于 `prepare_utils_zip`；
- **失败策略**要明确：缺了会构建失败的就 `process.exit`，只是功能降级就告警继续（现有 codex / rsshub 是后者）。

### 10.8 调整拆分粒度

改 `utils_zip_support.rs` 顶部：

```rust
const SPLIT_THRESHOLD_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SPLIT_CHILDREN: usize = 64;
const MAX_SPLIT_DEPTH: usize = 6;
```

改完建议：

```bash
rm -rf src-tauri/utils/__bundles      # 让新旧两种拆法不要同时存在
npm run prepare:tauri-utils
```

（`prune_stale_bundles` 本身会清理不再需要的包，但先清干净更省心，能顺带释放几 GB。）

### 10.9 改归档格式或清单字段

`BUNDLE_FORMAT_VERSION`（当前 `2`）是跨构建端/安装端的契约：

- 加**可选**字段：`#[serde(default)]` + 保持版本号（旧清单仍可读）；
- 改语义/去掉字段/换压缩方式：**必须** +1；
- 版本不匹配时安装端会直接报「资源清单版本不兼容 / 请使用完整的安装包」，即老 helper 不认新清单——所以**安装器与分包必须同批发布**，不要把新分包配旧安装器。

发版自检：`verify_utils_bundle_roundtrip` 会跑「建样例树 → 打包 → 解包 → 校验结构 + 跳过规则」，是最快的回归。

### 10.10 前端静态资源

- Excalidraw / Vditor：`tools/copy_excalidraw_assets.mjs`（`public/excalidraw`、`public/vditor`），`postinstall` 也会跑一次；
- 浏览器预览页的图表模块：改过 `guaxiChartData` / `guaxiChartTypes` / `guaxiChartSamples` 后必须重跑 `tools/build_chart_preview_bundle.mjs`，产物 `src-tauri/assets/vendor/guaxi-chart-preview.js` 会被 Rust `include_str!` 打进二进制（构建期已自动执行，手工改前端时记得别忘）。

### 10.11 改完怎么验证（按顺序）

1. **分包正确性**
   ```bash
   npm run prepare:tauri-utils
   ```
   看日志：`bundleCount` / `rebuilt=` / `reused=` / `totalBytes=` 是否符合预期。

2. **清单内容**
   ```bash
   grep -o '"source":"[^"]*"' src-tauri/utils/__bundles/bundle-manifest.json | sort -u | head
   ```
   确认新资源在、被排除的不在。

3. **打包/解包往返**
   ```bash
   cargo run --manifest-path src-tauri/Cargo.toml --bin verify_utils_bundle_roundtrip
   ```
   关注 `smoke test passed`。

4. **完整出包**
   ```bash
   npm run tauri -- build
   ```
   日志里应有 `[utils.bundle] prepared ...` 与 `[utils.bundle] published external windows bundles ...`。

5. **安装验证（必须做）**
   - 在**干净目录/干净机器**安装；
   - 确认 `$INSTDIR\utils\<新资源>` 存在、`$INSTDIR\.guaxi_utils_extract_complete` 已写；
   - 启动应用，走一遍依赖新资源的功能；
   - 再装一次（覆盖安装），确认增量路径正常（日志里出现 `skippedBundles`）。

---

## 11. 排错速查

| 现象 | 大概率原因 | 处理 |
|---|---|---|
| `error mmapping datablock` / NSIS 打包失败 | 又把大资源塞进了 `bundle.resources` | 走外置分包，别改回内嵌 |
| 安装弹「解压外部 utils 资源包失败」 | `utils_bundles/` 缺失、缺 `bundle-manifest.json`、或与安装器不同版本 | 一起分发；核对 `archive_blake3` |
| 日志一直是 `reusing existing bundle`，新文件没进包 | 用了 `reuse` 命令，没重扫 `utils` | 跑 `npm run prepare:tauri-utils` 或完整 build |
| 改了文件但包没更新 | 判断依据是 `path+size+mtime` | 确认 mtime 真的变了；必要时删掉对应 `*.tar.zst` |
| dev 启动后报「资源缺失」但文件明明在 | 代码绕过了 `ensure_relative_dir()` | 统一走统一入口；debug 下还会回退源码树 |
| dev 每次都被重启编译 | 运行期写入的目录没进 `.taurignore` | 补忽略项 |
| cargo 卡在下载 sherpa 库 | 预取没成功，build.rs 回去联网 | `npm run sherpa:fetch`（看代理日志） |
| `could not find native static library` | 把 `SHERPA_ONNX_LIB_DIR` 挂到了全局 env | 只给最终 tauri 调用注入（编排层已处理） |
| GPU 版跑起来还是 CPU | 缺 CUDA 运行时 DLL / 驱动 | `npm run sherpa:fetch:cuda`；看自检退出码 |
| 自检退出码 2 | 安装目录缺 CUDA DLL（打包缺陷） | 重新装配后重打，别发布 |
| 「资源清单版本不兼容」 | 新分包配了旧 helper | 安装器与分包必须同批发布 |
| release 链接阶段 OOM | Windows 内存峰值 | 确认 `releaseCargoEnv()` 生效（job=1 / incremental=0） |

---

## 12. 维护红线

1. 所有 `utils/...` 访问必须走 `ensure_relative_dir()`；新增代码不要自建路径解析。
2. 任何多 GB 资源都不许回到 `bundle.resources`；要外置就按 10.6 那一套加。
3. 新增资源前先问：它该在 `utils` 里，还是该独立分发？选错会导致安装后路径错位或包体暴涨。
4. 改 `SKIPPED_DIRECTORIES` / `should_skip()` / `SPLIT_*` / `BUNDLE_FORMAT_VERSION`，都要连带更新单测与本文。
5. 重命名 `prepare_utils_zip` 之类入口时，需同步：`package.json`、`tools/run_tauri_with_utils.mjs`、`build.rs`、CI 与本文。
6. `installer-hooks.nsh` 里任何新增步骤都必须**失败即 `Abort`**（字体安装这种可降级的除外），不允许静默跳过。
