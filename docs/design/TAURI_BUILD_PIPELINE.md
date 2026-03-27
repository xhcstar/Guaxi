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

### 2.2 Rust 构建与资源分包

- [`src-tauri/build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)
  Cargo 构建脚本，在 release 编译前兜底生成外部资源分包。
- [`src-tauri/utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs)
  虽然文件名沿用历史命名，但当前实际负责 `tar.zst` 的分包与解包。
- [`src-tauri/src/bin/prepare_utils_zip.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/bin/prepare_utils_zip.rs)
  手动或脚本显式触发资源分包的命令行入口。
- [`src-tauri/src/bin/extract_utils_bundles.rs`](/e:/Record/source/Vue/guaxi/src-tauri/src/bin/extract_utils_bundles.rs)
  Windows 安装阶段用于解包外部资源的 helper 可执行程序。

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
3. 因为 `dev` 不属于 `build/bundle`，所以不会触发：
   - `copy_excalidraw_assets.mjs`
   - `prepare_utils_zip`
   - `extract_utils_bundles` 的 release 构建
4. 脚本直接调用本地 `tauri dev`。
5. Tauri 读取 [`src-tauri/tauri.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.conf.json)：
   - `beforeDevCommand = npm run dev`
   - `devUrl = http://localhost:1420`
6. Tauri 启动 Vite 开发服务器。
7. Tauri 通过 Cargo 启动 Rust 主程序 `guaxi`。
8. Cargo 在编译主程序前调用 [`src-tauri/build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)。
9. `build.rs` 检测到 `PROFILE=debug`，输出跳过日志，不生成 `utils` 分包。

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
3. 先执行资源预处理：
   - `node ./tools/copy_excalidraw_assets.mjs`
4. 再执行资源分包：
   - `cargo run --manifest-path src-tauri/Cargo.toml --bin prepare_utils_zip`
5. `prepare_utils_zip` 调用 [`utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs) 的 `prepare_utils_bundles()`。
6. `prepare_utils_bundles()` 会扫描 [`src-tauri/utils`](/e:/Record/source/Vue/guaxi/src-tauri/utils)，按顶层目录或文件拆成多个 `*.tar.zst`。
7. 这些分包生成到：
   - [`src-tauri/utils/__bundles`](/e:/Record/source/Vue/guaxi/src-tauri/utils/__bundles)
8. 如果当前是 Windows，包装脚本还会构建：
   - `extract_utils_bundles.exe`
9. 之后脚本正式调用 `tauri build`。
10. Tauri 先执行 [`src-tauri/tauri.conf.json`](/e:/Record/source/Vue/guaxi/src-tauri/tauri.conf.json) 中的：
    - `beforeBuildCommand = npm run build`
11. 前端 `dist` 产物生成后，Cargo 开始 release 编译。
12. Cargo 在编译前执行 [`build.rs`](/e:/Record/source/Vue/guaxi/src-tauri/build.rs)。
13. `build.rs` 在 release 模式下再次调用 `prepare_utils_bundles()` 作为兜底。
14. Tauri 继续生成 NSIS 安装器。
15. 打包成功后，包装脚本把外部资源和 helper 复制到安装器同级目录：
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

---

## 6. tar.zst 分包方案

### 6.1 为什么选择 tar.zst

相比之前的 zip 方案，`tar.zst` 更适合当前项目的大型运行时资源目录：

- `tar` 可以先把大量小文件组织为连续流，降低碎文件开销
- `zstd` 压缩和解压都更快
- Rust 生态支持成熟，便于直接在构建链和安装 helper 中共用
- 可以通过多线程与多分包并行进一步提速

### 6.2 分包策略

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

### 6.3 明确排除的目录

以下目录不会进入 release 分包：

- `utils/MarkdownConvertor`
- `utils/Models/surya_models`

这些规则定义在 [`utils_zip_support.rs`](/e:/Record/source/Vue/guaxi/src-tauri/utils_zip_support.rs) 的 `SKIPPED_DIRECTORIES` 中。

---

## 7. 安装阶段如何解包

### 7.1 Windows 安装时机

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

### 7.2 为什么使用独立 Rust helper

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

### 7.3 安装时的目录结构

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

## 8. 运行时恢复机制

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

## 9. 复用已有分包

当 `utils` 内容没有变化时，可以复用上一次生成的外部分包，节省很多时间。

命令：

```bash
npm run tauri -- build --reuse-utils-zip
```

行为：

- 外层 JS 包装脚本把复用意图传给 `prepare_utils_zip`
- 同时设置 `GUAXI_REUSE_UTILS_ZIP=1`
- `build.rs` 也会复用已存在的分包，避免 Cargo 编译阶段又重新打包一遍

---

## 10. 常见问题

### 10.1 为什么 dev 模式不复制 utils

因为复制几 GB 资源会让每次开发启动都非常慢。当前通过运行时路径回退，直接复用源码目录下的 `src-tauri/utils`。

### 10.2 为什么 release 安装包旁边还要带额外文件

因为超大运行时资源不再嵌入 NSIS，而是改成外部分包，避免 NSIS 在超大资源场景下失败。

### 10.3 为什么参数名还是 `--reuse-utils-zip`

这是为了兼容之前已经使用过的脚本或命令习惯。虽然现在底层已经是 `tar.zst` 分包，参数名暂时没有改掉。

### 10.4 JSON 配置里为什么没有直接写详细注释

因为：

- `package.json`
- `tauri.conf.json`
- `tauri.windows.conf.json`
- `tauri.linux.conf.json`

都属于 JSON 文件。为了避免破坏解析兼容性，这些说明集中写在本文档和相邻代码注释中，而不是直接写进 JSON 文件。

---

## 11. 推荐使用方式

### 11.1 日常开发

```bash
npm run tauri -- dev
```

特点：

- 不打包 `utils`
- 不解包 `utils`
- 直接使用源码目录中的 `src-tauri/utils`

### 11.2 正式发布

首次完整构建：

```bash
npm run tauri -- build
```

复用已生成资源分包：

```bash
npm run tauri -- build --reuse-utils-zip
```

发布时请确保以下内容放在同一目录：

- `guaxi_*.exe`
- `utils_bundles/`
- `extract_utils_bundles.exe`
- `README_utils_bundles.txt`

---

## 12. 后续维护建议

如果后续继续扩展这套方案，建议优先遵守以下约束：

1. 所有访问 `utils/...` 的 Rust 代码都走 `ensure_relative_dir()`。
2. 任何新的大资源目录先评估是否要加入 `SKIPPED_DIRECTORIES` 或单独拆包。
3. 不要再把多 GB `utils` 直接重新塞回 NSIS `resources`。
4. 如果以后重命名 `prepare_utils_zip`，需要同时更新：
   - `package.json`
   - `tools/run_tauri_with_utils.mjs`
   - 相关发布脚本和文档
