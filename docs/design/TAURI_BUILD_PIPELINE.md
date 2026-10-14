# Guaxi Tauri 构建方案说明

本文档说明当前 Guaxi 桌面端在 `debug/dev` 与 `release/build` 两种模式下的构建、资源准备、安装与运行方案。

## 1. 目标

当前构建方案的主要目标有四个：

1. `debug/dev` 模式保持轻量，不在每次开发启动前复制或解压体积巨大的 `src-tauri/utils`。
2. `release/build` 模式避免把多 GB 的 `utils` 资源直接塞进 NSIS 安装包，绕开 NSIS 对超大内嵌资源的限制。
3. 保持运行时目录结构稳定。应用代码始终按 `utils/...` 的原始目录结构访问资源，不引入 `utils/utils/...` 之类的错层路径。
4. 在构建和安装阶段都提供可自动化、可复用、可恢复的工业化资源处理流程。

---

## 2. 关键文件

### 2.1 前端与 npm 入口

- [`package.json`](/e:/Record/source/Vue/guaxi/package.json)
  说明对外暴露的 npm 脚本。
- [`tools/run_tauri_with_utils.mjs`](/e:/Record/source/Vue/guaxi/tools/run_tauri_with_utils.mjs)
  统一封装 `tauri dev / build / bundle` 的外层入口。
- [`tools/copy_excalidraw_assets.mjs`](/e:/Record/source/Vue/guaxi/tools/copy_excalidraw_assets.mjs)
  预复制前端运行依赖的静态资源。
- [`tools/fetch_sherpa_onnx_libs.mjs`](/e:/Record/source/Vue/guaxi/tools/fetch_sherpa_onnx_libs.mjs)
  获取语音所需的原生库与模型；GPU 版还会生成 `onnxruntime.lib`（见第 6 节）。

### 2.2 Rust 构建与资源分包

- [`src-tauri/build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)
  Cargo 构建脚本，在 release 编译前兜底生成外部资源分包。
- [`src-tauri/utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs)
  虽然文件名沿用历史命名，但当前实际负责 `tar.zst` 的分包与解包。
- [`src-tauri/src/bin/prepare_utils_zip.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/bin/prepare_utils_zip.rs)
  手动或脚本显式触发资源分包的命令行入口。
- [`src-tauri/src/bin/extract_utils_bundles.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/bin/extract_utils_bundles.rs)
  Windows 安装阶段用于解包外部资源的 helper 可执行程序。
- [`src-tauri/.cargo/config.toml`](/e:/Record/source/Vue/guaxi/src-tauri/.cargo/config.toml)
  把 `SHERPA_ONNX_ARCHIVE_DIR` 指向预取的原生库归档，使构建期零联网。

### 2.3 运行时资源定位

- [`src-tauri/src/guaxiservices/bundled_resources.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/guaxiservices/bundled_resources.rs)
  统一处理 `utils/...` 的 debug/release 路径解析、恢复和按需解包。

### 2.4 打包与安装

- [`src-tauri/tauri.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.conf.json)
  Tauri 主配置，定义 `beforeDevCommand` 和 `beforeBuildCommand`。
- [`src-tauri/tauri.windows.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.windows.conf.json)
  Windows 打包配置。当前不再把 `utils` 大资源直接打进 NSIS。
- [`src-tauri/tauri.linux.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.linux.conf.json)
  Linux 打包配置。当前仍允许携带 `utils/__bundles/**/*`。
- [`src-tauri/installer-hooks.nsh`](/e:/Record/source/Vue/guaxi/src-tauri/installer-hooks.nsh)
  NSIS 安装后置钩子，调用解包 helper。

---

## 3. npm 脚本说明

### 3.1 开发相关

- `npm run dev`
  启动 Vite 前端开发服务器。
- `npm run tauri -- dev`
  启动 Tauri 开发模式。

### 3.2 构建相关

- `npm run build`
  构建前端生产资源。
- `npm run prepare:tauri-assets`
  复制 Excalidraw / Vditor 等静态资源。
- `npm run prepare:tauri-utils`
  手动生成 `src-tauri/utils/__bundles/*.tar.zst`。
- `npm run tauri -- build`
  执行完整 release 构建流程。
- `npm run tauri -- build --reuse-utils-zip`
  复用已存在的 `utils` 分包，不重新打包资源。
- `npm run tauri:build:reuse`
  与上一条等价，但更适合日常直接使用，避免 npm 参数转发写错。
- `npm run tauri:bundle:reuse`
  复用已存在的 `utils` 分包执行 bundle。

### 3.3 语音资源准备（新增）

- `npm run sherpa:fetch`
  获取语音所需的原生库与模型（幂等，已存在即跳过）。
- `npm run sherpa:fetch:force`
  忽略已有文件全部重下。
- `npm run sherpa:fetch:cuda`
  额外准备 GPU 版原生库（约 595MB 包 + 生成 `onnxruntime.lib`）。

### 3.4 语音功能的 GPU 构建（新增）

在常规命令后加 `-- --no-default-features --features gpu` 即可，例如：

```bash
npm run tauri -- dev -- --no-default-features --features gpu
npm run tauri -- build -- --no-default-features --features gpu
```

详见第 6 节。

说明：

- `--reuse-utils-zip` 这个参数名是历史兼容名称。
- 现在它的真实含义是“复用已生成的 `tar.zst` 分包”。

---

## 4. Debug / Dev 模式

### 4.1 入口命令

```bash
npm run tauri -- dev
```

### 4.2 调用关系

调用顺序如下：

1. `npm` 执行 [`tools/run_tauri_with_utils.mjs`](/e:/Record/source/Vue/guaxi/tools/run_tauri_with_utils.mjs)。
2. 脚本识别到子命令是 `dev`。
3. 脚本先跑两件**与子命令无关、dev 也要做**的准备（都幂等）：
   - `node ./tools/fetch_codex_sidecar.mjs`
   - `node ./tools/fetch_sherpa_onnx_libs.mjs`（原生库必须在 cargo 构建前就位，否则 build.rs 会去联网下载并卡住）
4. 因为 `dev` 不属于 `build/bundle`，所以不会触发：
   - `copy_excalidraw_assets.mjs`
   - `prepare_utils_zip`
   - `extract_utils_bundles` 的 release 构建
5. 脚本直接调用本地 `tauri dev`。
6. Tauri 读取 [`src-tauri/tauri.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.conf.json)：
   - `beforeDevCommand = npm run dev`
   - `devUrl = http://localhost:1420`
7. Tauri 启动 Vite 开发服务器。
8. Tauri 通过 Cargo 启动 Rust 主程序 `guaxi`。
9. Cargo 在编译主程序前调用 [`src-tauri/build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)。
10. `build.rs` 检测到 `PROFILE=debug`，输出跳过日志，不生成 `utils` 分包。

### 4.3 为什么 debug 不需要复制 utils

`debug/dev` 模式下并不会把 `src-tauri/utils` 复制到 `target/debug/utils` 或 exe 同级目录。

这是通过 [`bundled_resources.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/guaxiservices/bundled_resources.rs) 完成的：

1. 应用代码访问 `utils/...` 路径时，统一调用 `ensure_relative_dir("utils/...")`。
2. `ensure_relative_dir()` 会进一步进入 `ensure_utils_ready_internal()`。
3. 如果当前没有安装后的 `utils` 和外部分包，则在 debug 模式下调用 `dev_source_utils_root()`。
4. `dev_source_utils_root()` 会从当前 exe 路径往上寻找：
   - `Cargo.toml`
   - `utils` 目录
5. 找到后直接把源码目录下的 [`src-tauri/utils`](/e:/Record/source/Vue/guaxi/src-tauri/utils) 当作运行时资源根。

### 4.4 debug 模式的特点

- 优点：
  - 启动快
  - 不需要额外打包、复制、解压几 GB 资源
  - 本地修改 `src-tauri/utils` 后可直接被 dev 使用
- 约束：
  - 后台命令必须统一通过 `ensure_relative_dir()` 访问 `utils/...`
  - 如果某处绕过统一入口、手工拼接 exe 同级 `utils` 路径，debug 下就可能失败

---

## 5. Release / Build 模式

### 5.1 入口命令

```bash
npm run tauri -- build
```

或复用已生成分包：

```bash
npm run tauri -- build --reuse-utils-zip
```

### 5.2 调用关系

完整调用顺序如下：

1. `npm` 调用 [`tools/run_tauri_with_utils.mjs`](/e:/Record/source/Vue/guaxi/tools/run_tauri_with_utils.mjs)。
2. 包装脚本识别到子命令是 `build`。
3. 先跑两件与子命令无关的准备（幂等）：
   - `node ./tools/fetch_codex_sidecar.mjs`
   - `node ./tools/fetch_sherpa_onnx_libs.mjs`
     - 原生库要赶在 cargo 构建前就位；
     - 模型要赶在下一步 `prepare_utils_zip` 之前就位，否则不会进离线包。
     - 若带了 `--features gpu`，脚本还会把 `SHERPA_ONNX_LIB_DIR` 指向装配好的 CUDA 库目录。
4. 再执行资源预处理：
   - `node ./tools/copy_excalidraw_assets.mjs`
5. 再执行资源分包：
   - `cargo run --manifest-path src-tauri/Cargo.toml --bin prepare_utils_zip`
6. `prepare_utils_zip` 调用 [`utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs) 的 `prepare_utils_bundles()`。
7. `prepare_utils_bundles()` 会扫描 [`src-tauri/utils`](/e:/Record/source/Vue/guaxi/src-tauri/utils)，按顶层目录或文件拆成多个 `*.tar.zst`。
8. 这些分包生成到：
   - [`src-tauri/utils/__bundles`](/e:/Record/source/Vue/guaxi/src-tauri/utils/__bundles)
9. 如果当前是 Windows，包装脚本还会构建：
   - `extract_utils_bundles.exe`
   - 这一步会显式设置 `GUAXI_SKIP_BUNDLE_PREPARE=1`，避免 helper 的单独 release 编译再次触发 `build.rs` 重新打包 `utils` 分包。
10. 之后脚本正式调用 `tauri build`。
11. Tauri 先执行 [`src-tauri/tauri.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.conf.json) 中的：
    - `beforeBuildCommand = npm run build`
12. 前端 `dist` 产物生成后，Cargo 开始 release 编译。
13. Cargo 在编译前执行 [`build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)。
14. `build.rs` 在 release 模式下再次调用 `prepare_utils_bundles()` 作为兜底。
    - 如果设置了 `GUAXI_REUSE_UTILS_ZIP=1`，则尽量复用已有分包。
    - 如果设置了 `GUAXI_SKIP_BUNDLE_PREPARE=1`，则完全跳过分包准备，只执行正常的 Cargo/Tauri 构建流程。
15. Tauri 继续生成 NSIS 安装器。
16. 打包成功后，包装脚本把外部资源和 helper 复制到安装器同级目录：
    - `utils_bundles/*.tar.zst`
    - `extract_utils_bundles.exe`
    - `README_utils_bundles.txt`

### 5.3 为什么 release 不把 utils 直接打进安装包

原因是历史上将整个 `utils` 或其大型压缩包直接放进 NSIS 时，曾触发：

- 单文件 `mmap` 失败
- 安装器内部编译器超出范围
- 总资源体积接近或超过 NSIS 可稳定处理的边界

所以当前方案改为：

1. 主安装器只携带程序主体和较小资源。
2. 超大 `utils` 运行时资源作为安装器同级的外部分包分发。
3. 安装时再落地到最终 `utils` 目录。

### 5.4 Release 模式生成的外部文件

Windows 打包成功后，发布目录中应至少包含：

- `guaxi_*.exe`
- `utils_bundles/`
- `extract_utils_bundles.exe`
- `README_utils_bundles.txt`

其中：

- `utils_bundles` 存放 `*.tar.zst`
- `extract_utils_bundles.exe` 负责安装时解包
- `README_utils_bundles.txt` 说明如何一起分发

### 5.5 Windows release 构建的额外优化

由于当前 `guaxi` 的 release 构建体量很大，Windows 下的 `rustc/LLVM` 容易在链接或代码生成阶段出现内存不足。

为降低这类失败概率，包装脚本在 Windows 的 release 构建中会额外设置：

- `CARGO_BUILD_JOBS=1`
- `CARGO_INCREMENTAL=0`

目的如下：

- 将 release 构建串行化，减少并发 codegen 占用的峰值内存
- 禁用 incremental，避免额外缓存状态继续拉高内存和磁盘占用

对应实现位于 [`tools/run_tauri_with_utils.mjs`](/e:/Record/source/Vue/guaxi/tools/run_tauri_with_utils.mjs) 中的 `releaseCargoEnv()`。

---

## 6. 语音功能的构建特性（CPU / GPU）

语音能力已从「启动外部可执行文件」改为**进程内调用 sherpa-onnx 库**。原生库由 Rust 依赖
[`sherpa-onnx`](/e:/Record/source/Vue/guaxi/src-tauri/Cargo.toml) 及其 `-sys` 包提供，
通过 Cargo feature 在「CPU 静态库」与「GPU 动态库」之间切换。

### 6.1 两个 feature

| feature | 链接方式 | 特点 |
|---|---|---|
| 默认 `default = ["sherpa-onnx/static"]` | 静态 | 自包含，**无需分发任何 DLL**，任何机器都能构建 |
| `gpu` = `["sherpa-onnx/shared"]` | 动态 | 使用官方 CUDA 版运行库，支持 `provider=cuda` |

两者互斥：`static` 与 `shared` 同时打开时，`sherpa-onnx-sys` 的 build.rs 会直接报错。
因此 **GPU 构建必须带 `--no-default-features`**。定义位于
[`src-tauri/Cargo.toml`](/e:/Record/source/Vue/guaxi/src-tauri/Cargo.toml) 的 `[features]`。

### 6.2 依赖准备

语音的模型与原生库都不进 git，由
[`tools/fetch_sherpa_onnx_libs.mjs`](/e:/Record/source/Vue/guaxi/tools/fetch_sherpa_onnx_libs.mjs) 获取：

```bash
npm run sherpa:fetch        # 缺什么补什么（幂等）
npm run sherpa:fetch:force  # 全部重下
npm run sherpa:fetch:cuda   # 额外准备 GPU 版原生库
```

该脚本在 `dev / build / bundle` 时都会由
[`tools/run_tauri_with_utils.mjs`](/e:/Record/source/Vue/guaxi/tools/run_tauri_with_utils.mjs)
**自动调用**（幂等，已存在即跳过），正常流程不必手动执行。它做四件事：

1. 取 sherpa-onnx 的 Windows 静态库归档到 `src-tauri/.sherpa-onnx-libs/<ver>/`；
2. 取模型（Kokoro TTS / SenseVoice ASR / whisper ONNX / Silero VAD）到 `src-tauri/utils/Models/`；
3. 下载走 curl，代理按「`*_PROXY` 环境变量 → Windows 注册表系统代理」依次解析；
4. 大文件支持断点续传（保留 `.part`，失败不整包重下）。

**为什么构建期不能让 Cargo 自己下**：`sherpa-onnx-sys` 的 build.rs 默认会自行去 GitHub
下载预编译库，而构建机常常只有注册表里的系统代理、没有 `*_PROXY` 环境变量，cargo 会卡死。
所以改由脚本预先取好，构建时经
[`src-tauri/.cargo/config.toml`](/e:/Record/source/Vue/guaxi/src-tauri/.cargo/config.toml)
的 `SHERPA_ONNX_ARCHIVE_DIR` 指过去，**构建期零联网**。

### 6.3 CPU 构建（默认）

不需要任何额外参数，dev 与 release 一致：

```bash
# dev
npm run tauri -- dev

# release（完整构建）
npm run tauri -- build

# release（复用已有 utils 分包）
npm run tauri:build:reuse
```

### 6.4 GPU 构建

先准备 CUDA 版原生库（只需一次）：

```bash
npm run sherpa:fetch:cuda
```

之后在 cargo 参数上加两个开关即可：

```bash
# dev
npm run tauri -- dev -- --no-default-features --features gpu

# release
npm run tauri -- build -- --no-default-features --features gpu
```

`--` 之后的参数由 Tauri 转发给 cargo。

**不需要手动设 `SHERPA_ONNX_LIB_DIR`**：包装脚本检测到 `--features gpu` 会自动把它指向
装配好的 CUDA 库目录；目录不存在时会立即报错并提示先跑 `npm run sherpa:fetch:cuda`。

直接使用 cargo（不经过包装脚本）时才需要自己设：

```bash
SHERPA_ONNX_LIB_DIR=src-tauri/.sherpa-onnx-libs/1.13.8/cuda-lib \
  cargo build --no-default-features --features gpu
```

### 6.5 GPU 支持的原理：为什么要自己造 `onnxruntime.lib`

官方提供 CUDA 预编译包
`sherpa-onnx-v<ver>-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-win-x64-cuda.tar.bz2`，
但它**不能直接拿来链接**。包内 `lib/` 只有两类东西：

- `sherpa-onnx-c-api.lib` / `sherpa-onnx-cxx-api.lib`：DLL 的导入库
- `onnxruntime.dll`、`onnxruntime_providers_cuda.dll` 等：运行时 DLL

而 `sherpa-onnx-sys` 的动态链接模式会发出两条链接指令：

```text
cargo:rustc-link-lib=dylib=sherpa-onnx-c-api
cargo:rustc-link-lib=dylib=onnxruntime
```

第二条要求 `onnxruntime.lib` 存在，而 CUDA 包里没有这个文件，所以直接链接必然失败。

注意不要想着「把静态包里那个 `onnxruntime.lib` 拷过来」——两者同名但**不是一回事**：
静态包里的是**静态库**（配合 `-l static=onnxruntime` 把所有代码链进 exe），
这里需要的是 `onnxruntime.dll` 的**导入库**，负责把对 DLL 的调用重定向过去。
混用不会得到你要的结果。

**解法：从 `onnxruntime.dll` 反推出导入库。** 这个 DLL 只导出 4 个符号：

```text
OrtGetApiBase
OrtSessionOptionsAppendExecutionProvider_CPU
OrtSessionOptionsAppendExecutionProvider_CUDA
OrtSessionOptionsAppendExecutionProvider_Tensorrt
```

其中 `OrtSessionOptionsAppendExecutionProvider_CUDA` 的存在说明
**CUDA provider 确实已经编进这个 DLL**，这也正是链接时必须提供这份导入库的原因。

`npm run sherpa:fetch:cuda` 会自动完成三步：

1. `dumpbin /exports onnxruntime.dll` 取导出表；
2. 由导出表生成 `.def` 文件；
3. `lib /def:... /machine:x64 /out:onnxruntime.lib` 产出导入库。

需要用 MSVC 的 `dumpbin` / `lib`（脚本用 `vswhere` 定位；找不到会明确跳过并提示）。
最终装配出的 `src-tauri/.sherpa-onnx-libs/<ver>/cuda-lib/` **同时含导入库与 DLL**——
DLL 放在一起是有意的：shared 模式下 `sherpa-onnx-sys` 的 build.rs 会把 lib 目录里的 DLL
自动拷到可执行文件旁，省得再单独分发。

### 6.6 运行时的 provider 选择：必须先探测再请求

`provider=cuda` 有一个必须提前规避的坑：**CUDA 组件缺失时，onnxruntime 抛出的 C++ 异常会
穿透 FFI 边界，Rust 捕不住，直接 abort 整个进程**（实测报 `Rust cannot catch foreign exceptions`）。

也就是说，「先试 cuda、失败再回退 cpu」这种写法**根本来不及执行回退，进程已经死了**。
所以运行时改为**先探测再请求**（见 `src/guaxiservices/speech/config.rs` 的 `cuda_available()`）：
只有 NVIDIA 驱动（`nvcuda.dll`）与 `onnxruntime_providers_cuda.dll` 都在时才会请求 cuda，
否则在构造引擎之前就降级到 cpu，并打一条 warn 日志。

### 6.7 release 打包注意

- **CPU 构建**：静态库已链进 exe，`utils` 里不需要任何语音相关 DLL。
- **GPU 构建**：`onnxruntime*.dll`（含约 496MB 的 `onnxruntime_providers_cuda.dll`）
  会被 build.rs 拷到 exe 旁，打包时需一并分发。
- 语音的**外部可执行文件目录已删除**（`utils/sherpa-onnx`、`utils/whisper`），
  并在 [`utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs)
  的 `SKIPPED_DIRECTORIES` 中排除；下载归档（`*.tar.bz2`）同样不进包。

---

## 7. tar.zst 分包方案

### 7.1 为什么选择 tar.zst

相比之前的 zip 方案，`tar.zst` 更适合当前项目的大型运行时资源目录：

- `tar` 可以先把大量小文件组织为连续流，降低碎文件开销
- `zstd` 压缩和解压都更快
- Rust 生态支持成熟，便于直接在构建链和安装 helper 中共用
- 可以通过多线程与多分包并行进一步提速

### 7.2 分包策略

当前策略是按 `src-tauri/utils` 的顶层目录/文件拆分。

例如：

- `utils/Models` -> `Models.tar.zst`
- `utils/SlidevEnv` -> `SlidevEnv.tar.zst`
- `utils/tool.exe` -> `tool.exe.tar.zst`

这样做有几个好处：

- 单个 bundle 更容易排查和替换
- 构建时可以并行压缩多个 bundle
- 安装时可以并行解包多个 bundle
- 最终解包后的目录仍保持原始 `utils/...` 结构

### 7.3 明确排除的目录

以下目录不会进入 release 分包：

- `utils/MarkdownConvertor`
- `utils/Models/surya_models`
- `utils/MinerU`、`utils/Models/mineru_models`
- `utils/sherpa-onnx`、`utils/whisper`（语音已改为进程内调用库，外部可执行文件已移除）

此外，`utils` 下任何 `*.tar.bz2` 下载归档也不会进包：它们只是解压用的中间产物，
留在目录里会把 release 白撑大几百 MB。

这些规则定义在 [`utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs) 的 `SKIPPED_DIRECTORIES` 与 `should_skip()` 中。

---

## 8. 安装阶段如何解包

### 8.1 Windows 安装时机

Windows 安装器使用 [`installer-hooks.nsh`](/e:/Record/source/Vue/guaxi/src-tauri/installer-hooks.nsh)。

`NSIS_HOOK_POSTINSTALL` 在主程序文件复制完成后调用：

```text
extract_utils_bundles.exe --bundle-root "$EXEDIR\utils_bundles" --destination "$INSTDIR\utils"
```

含义如下：

- `$EXEDIR`
  安装器所在目录，也就是发布时外部 bundle 目录所在位置
- `$INSTDIR`
  应用最终安装目录

### 8.2 为什么使用独立 Rust helper

之前尝试过：

- NSIS 内联 PowerShell
- `Expand-Archive`

这些方案存在：

- 语法转义脆弱
- 解压速度慢
- 对大型资源处理不够稳定

因此当前改成独立 helper：

- 避免多层 shell / PowerShell 引号问题
- 与构建期共用 Rust 资源处理逻辑
- 更容易调试和扩展

### 8.3 安装时的目录结构

解包目标是：

- `$INSTDIR/utils`

解包后目录必须保持：

- `utils/Models/...`
- `utils/SlidevEnv/...`
- `utils/llama/...`

而不是：

- `utils/utils/...`

当前 `tar` 中保存的是原始相对路径，因此解包后结构保持不变。

---

## 9. 运行时恢复机制

即使安装或首次解包中途中断，运行时也有恢复逻辑。

对应代码在 [`bundled_resources.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/guaxiservices/bundled_resources.rs)。

恢复机制包含：

1. 临时目录解包
   - 先解包到 `utils.__extracting_<pid>/__payload`
2. 完成标记
   - 写入 `.guaxi_extract_complete`
3. 正式落地
   - 再移动到最终 `utils`
4. 完成标记
   - 写入 `.guaxi_utils_extract_complete`
5. 下次启动恢复
   - 如果发现中间态目录，会尝试恢复并清理

这样即使中途退出，也尽量避免用户看到一个半解压的 `utils` 目录。

---

## 10. 复用已有分包

当 `utils` 内容没有变化时，可以复用上一次生成的外部分包，节省很多时间。

命令：

```bash
npm run tauri -- build --reuse-utils-zip
```

或直接使用固定脚本：

```bash
npm run tauri:build:reuse
```

行为：

- 外层 JS 包装脚本把复用意图传给 `prepare_utils_zip`
- 同时设置 `GUAXI_REUSE_UTILS_ZIP=1`
- `build.rs` 会优先复用已存在的分包，避免 Cargo 编译阶段又重新打包一遍
- Windows 下单独编译 `extract_utils_bundles.exe` 时，也会继承复用环境变量

如果使用 `reuse` 仍然看到日志里出现：

```text
[utils.bundle] prepared ...
```

而不是：

```text
[utils.bundle] reusing existing ...
```

则通常意味着以下情况之一：

1. `src-tauri/utils/__bundles` 目录下没有可复用的 `*.tar.zst`
2. 参与构建的某个子流程没有正确继承 `GUAXI_REUSE_UTILS_ZIP`
3. 你运行的命令没有把参数真正转发给包装脚本

---

## 11. 常见问题

### 11.1 为什么 dev 模式不复制 utils

因为复制几 GB 资源会让每次开发启动都非常慢。当前通过运行时路径回退，直接复用源码目录下的 `src-tauri/utils`。

### 11.2 为什么 release 安装包旁边还要带额外文件

因为超大运行时资源不再嵌入 NSIS，而是改成外部分包，避免 NSIS 在超大资源场景下失败。

### 11.3 为什么参数名还是 `--reuse-utils-zip`

这是为了兼容之前已经使用过的脚本或命令习惯。虽然现在底层已经是 `tar.zst` 分包，参数名暂时没有改掉。

### 11.4 为什么 `prepare_utils_zip` 这个名字还保留

当前底层实现已经不再生成单个 `utils.zip`，而是生成：

- `src-tauri/utils/__bundles/*.tar.zst`

但以下名字仍然保留了历史命名：

- `prepare_utils_zip` 二进制
- `prepare:tauri-utils` npm 脚本
- `--reuse-utils-zip` 参数
- `GUAXI_REUSE_UTILS_ZIP` 环境变量

保留这些历史命名的原因主要是兼容性：

1. 避免打断你已经在用的本地命令、构建脚本和 CI 调用方式
2. 避免一次性重命名过多入口，导致排查问题时“名字变了但逻辑没变”
3. 让旧的使用习惯仍然有效，再逐步把文档语义迁移到新的 `tar.zst` 方案上

可以把它理解成：

- 名字还是 “zip”
- 实际行为已经是 “prepare external tar.zst bundles”

也就是说，当前这些名称都属于“历史兼容入口”，不代表真实产物格式。

### 11.5 为什么 helper 编译也会影响分包复用

Windows 发布流程中，安装阶段需要单独的 `extract_utils_bundles.exe`。

因此构建时除了主应用 `tauri build` 外，还会额外执行一次：

```text
cargo build --release --bin extract_utils_bundles
```

这个命令同样会触发 `build.rs`。如果不额外传入：

- `GUAXI_REUSE_UTILS_ZIP=1`
- 或 `GUAXI_SKIP_BUNDLE_PREPARE=1`

那么即使主流程已经准备好分包，helper 编译也可能再次触发分包生成。

当前包装脚本已经显式处理了这一点。

### 11.6 JSON 配置里为什么没有直接写详细注释

因为：

- `package.json`
- `tauri.conf.json`
- `tauri.windows.conf.json`
- `tauri.linux.conf.json`

都属于 JSON 文件。为了避免破坏解析兼容性，这些说明集中写在本文档和相邻代码注释中，而不是直接写进 JSON 文件。

---

## 12. 推荐使用方式

### 12.1 日常开发

```bash
npm run tauri -- dev
```

特点：

- 不打包 `utils`
- 不解包 `utils`
- 直接使用源码目录中的 `src-tauri/utils`
- 会自动确保语音的原生库与模型就位（幂等）

### 12.2 正式发布

首次完整构建：

```bash
npm run tauri -- build
```

复用已生成资源分包：

```bash
npm run tauri -- build --reuse-utils-zip
```

更推荐的等价命令：

```bash
npm run tauri:build:reuse
```

发布时请确保以下内容放在同一目录：

- `guaxi_*.exe`
- `utils_bundles/`
- `extract_utils_bundles.exe`
- `README_utils_bundles.txt`

### 12.3 使用 GPU 的构建

首次需要先准备 CUDA 版原生库：

```bash
npm run sherpa:fetch:cuda
```

之后在常规命令后追加 `-- --no-default-features --features gpu`：

```bash
# 开发
npm run tauri -- dev -- --no-default-features --features gpu

# 发布
npm run tauri -- build -- --no-default-features --features gpu
```

要点：

- 环境变量由包装脚本自动处理，不必手工设 `SHERPA_ONNX_LIB_DIR`；
- GPU 构建产物依赖 `onnxruntime*.dll`，发布时需随 exe 一起分发；
- 没有 NVIDIA 卡时，运行时探测会自动降级到 CPU，不会崩。

---

## 13. 后续维护建议

如果后续继续扩展这套方案，建议优先遵守以下约束：

1. 所有访问 `utils/...` 的 Rust 代码都走 `ensure_relative_dir()`。
2. 任何新的大资源目录先评估是否要加入 `SKIPPED_DIRECTORIES` 或单独拆包。
3. 不要再把多 GB `utils` 直接重新塞回 NSIS `resources`。
4. 如果以后重命名 `prepare_utils_zip`，需要同时更新：
   - `package.json`
   - `tools/run_tauri_with_utils.mjs`
   - 相关发布脚本和文档
