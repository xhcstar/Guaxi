# Guaxi Capture 移动端 App 构建与交互设计

## 目标

Guaxi Capture 是一个独立于 Guaxi PC 主程序的采集 App，代码位于 `apps/guaxi-capture`。它的目标是把手机端、浏览器端、剪贴板、图片 OCR 等来源统一整理成 Markdown 草稿，再通过 Guaxi PC 端开放的 Capture Import API 写入文件库。

设计重点：

- App 代码独立，不和 Guaxi PC 主程序的 `src`、`src-tauri` 混放。
- 移动端优先支持 Android 和 iOS 构建。
- Guaxi PC 端作为文件库、OCR 服务和导入服务的承载端。
- Capture App 只负责采集、预览、编辑、离线队列和提交。
- 所有入口统一归一化为同一份草稿结构，避免每个平台各写一套转换逻辑。

## 目录结构

```text
apps/guaxi-capture/
  browser-extension/              # Chrome / Edge 扩展，用于网页采集
  scripts/check-mobile-env.ps1    # Windows Android 构建环境检查
  src/
    main.js                       # Capture App UI 和交互逻辑
    styles.css                    # Capture App 样式
    services/
      captureClient.js            # 发送到 Guaxi PC Capture Import API
      htmlToMarkdown.js           # 轻量 HTML -> Markdown 转换
      ocrClient.js                # OCR 调用封装
      shareIntake.js              # 外部分享 payload 归一化
  src-tauri/
    src/lib.rs                    # Tauri 壳、启动参数、本地接收器、OCR 代理
    tauri.conf.json               # Tauri 配置
```

相关文档：

- `docs/capture-import-api.md`：Guaxi PC 端导入 API。
- `.github/workflows/guaxi-capture-ios.yml`：GitHub Actions iOS 构建流程。

## 功能总览

当前实现的功能：

- 手动编辑 Markdown。
- 从 URL 参数生成草稿。
- 从剪贴板读取文本或 HTML。
- 接收 Tauri 原生事件 `guaxi-capture-incoming`。
- 桌面壳支持启动参数 `--capture-json`、`--capture-text`、`--capture-url`。
- 桌面壳支持本地 HTTP 接收器 `127.0.0.1:14230/capture`。
- 浏览器扩展提取当前网页或选中内容，并发送到 Capture App。
- HTML 自动转换为 Markdown。
- 图片附件读取为 base64。
- 图片 OCR 并追加到 Markdown。
- 离线队列，发送失败时保存草稿，稍后重试。
- 复制 Markdown。
- 导出 Markdown。
- 发送到 Guaxi PC 文件库。

后续移动端可继续补充：

- Android Share Intent。
- iOS Share Extension。
- 移动端原生相机入口。
- 移动端本机 OCR。
- 用户登录、远程同步、端到端分享权限。

## 交互架构

```text
知乎 / 微信公众号 / 小红书 / B站 / 浏览器选区
            |
            | browser-extension / Share Intent / Clipboard / URL params
            v
Guaxi Capture App
  shareIntake.js        # 归一化 payload
  htmlToMarkdown.js     # HTML -> Markdown
  ocrClient.js          # 图片 OCR
  captureClient.js      # 提交 Guaxi PC
            |
            | POST /extension/capture/import
            v
Guaxi PC 后端
  chrome_extension_service.rs
  repoview::add_imported_file
            |
            | emit repo_imported_files_changed
            v
Guaxi PC 前端文件库刷新
```

## Capture App 草稿模型

前端内部以草稿为核心数据结构，默认由 `createCaptureDraft()` 创建：

```js
{
  id,
  title,
  sourceUrl,
  sourceApp,
  description,
  knowledgeBase,
  fileRepo,
  tagsText,
  markdown,
  assets,
  status,
  createdAt,
  updatedAt,
  lastError
}
```

设计思路：

- `tagsText` 是 UI 友好的逗号字符串，提交前转换为 `tags` 数组。
- `assets` 保存 `{ name, mime, dataBase64 }`，既可作为 Guaxi 导入附件，也可作为 OCR 输入。
- `status` 和 `lastError` 用于离线队列。
- 草稿只在 Capture App 内存在，真正写入文件库由 Guaxi PC API 完成。

## 外部分享归一化

实现位置：

```text
apps/guaxi-capture/src/services/shareIntake.js
```

支持字段别名：

| 目标字段 | 支持别名 |
| --- | --- |
| `title` | `title`, `name` |
| `sourceUrl` | `sourceUrl`, `source_url`, `url`, `link` |
| `sourceApp` | `sourceApp`, `source_app`, `source`, `app`, `from` |
| `description` | `description`, `desc`, `summary` |
| `knowledgeBase` | `knowledgeBase`, `knowledge_base`, `kb` |
| `fileRepo` | `fileRepo`, `file_repo`, `repo`, `library` |
| `tagsText` | `tagsText`, `tags`, `tag` |
| `markdown` | `markdown`, `md`, `content`, `text`, `body` |
| `html` | `html`, `articleHtml`, `article_html`, `selectionHtml`, `selection_html` |

来源平台识别：

- `mp.weixin.qq.com`、`weixin.qq.com` -> `wechat`
- `zhihu.com` -> `zhihu`
- `xiaohongshu.com`、`xhslink.com` -> `xiaohongshu`
- `bilibili.com`、`b23.tv` -> `bilibili`
- OCR / 相机入口 -> `camera`
- 剪贴板入口 -> `clipboard`

关键日志：

```text
[capture-intake]
```

## HTML 转 Markdown

实现位置：

```text
apps/guaxi-capture/src/services/htmlToMarkdown.js
```

当前转换器是轻量实现，依赖浏览器原生 `DOMParser`，不引入额外包。支持：

- 标题：`h1` - `h6`
- 段落：`p`
- 换行：`br`
- 分割线：`hr`
- 引用：`blockquote`
- 代码块：`pre`
- 行内代码：`code`
- 加粗、斜体、删除线
- 链接：`a`
- 图片：`img`
- 有序/无序列表
- 表格

设计思路：

- 浏览器扩展和移动端分享只需要提供 HTML，不需要自己做 Markdown 转换。
- 转换逻辑集中在 App 内，后续可以替换为 Readability + Turndown。
- 当前转换器优先保证内容可编辑和结构基本保留。

关键日志：

```text
[capture-markdown]
```

## 图片 OCR

实现位置：

```text
apps/guaxi-capture/src/services/ocrClient.js
apps/guaxi-capture/src-tauri/src/lib.rs
```

Guaxi PC 主程序已有 OCR command：

```text
src-tauri/src/commands/ocr.rs
commands::ocr::ocr_recognize
```

该 command 实际调用本机 agent_server：

```text
http://127.0.0.1:9876/ocr
```

Capture App 没有直接依赖 Guaxi PC 主程序 Rust crate，而是复用同一个 HTTP OCR 服务。Tauri 壳内提供 `capture_ocr_recognize` command 做代理：

```rust
#[tauri::command]
fn capture_ocr_recognize(endpoint: String, image_base64: String) -> Result<Value, String>
```

设计思路：

- 桌面/Tauri 环境优先走 Rust 代理，避免浏览器 CORS 问题。
- 普通浏览器预览时 fallback 到 `fetch(endpoint)`。
- OCR 输入为图片 base64，兼容 Guaxi PC 的 OCR 端点参数。
- OCR 结果会以 Markdown 追加到当前草稿。

移动端注意：

- 手机上的 `127.0.0.1` 指手机本机，不是电脑。
- 如果 OCR 服务仍运行在 PC，需要把 OCR endpoint 改成电脑局域网 IP：

```text
http://你的电脑IP:9876/ocr
```

关键日志：

```text
[capture-ocr]
```

## 发送到 Guaxi PC 文件库

实现位置：

```text
apps/guaxi-capture/src/services/captureClient.js
src-tauri/src/guaxiservices/chrome_extension_service.rs
src-tauri/src/commands/repoview.rs
```

默认提交地址：

```text
http://127.0.0.1:14219/extension/capture/import
```

移动端真机需要改成：

```text
http://你的电脑IP:14219/extension/capture/import
```

提交 payload：

```json
{
  "title": "文章标题",
  "sourceUrl": "https://example.com/article",
  "sourceApp": "zhihu",
  "description": "摘要",
  "knowledgeBase": "default",
  "fileRepo": "Inbox",
  "tags": ["capture", "zhihu"],
  "createdAt": "2026-07-04 18:00:00",
  "markdown": "# Markdown 内容",
  "assets": [
    {
      "name": "image.png",
      "mime": "image/png",
      "dataBase64": "..."
    }
  ]
}
```

PC 端处理流程：

1. `POST /extension/capture/import` 接收 payload。
2. 归一化标题、来源、标签、正文。
3. 将 Markdown 写入临时 staging 目录。
4. 通过 `add_web_clipper_import_file` 导入到 Libraries。
5. 将附件写入导入文档的 `images` 目录。
6. 调用 `add_imported_file` 写入数据库。
7. 发送 `repo_imported_files_changed` 事件。
8. Guaxi PC 前端文件库监听事件并刷新。

关键日志：

```text
[capture_import]
[capture]
repo_imported_files_changed
```

## 离线队列

实现位置：

```text
apps/guaxi-capture/src/services/captureClient.js
apps/guaxi-capture/src/main.js
```

本地存储 key：

```text
guaxi_capture_endpoint
guaxi_capture_queue
```

行为：

- 发送失败时，草稿进入队列，状态为 `failed`。
- 用户可点击同步队列重新发送。
- 发送成功后从队列移除。
- 队列仅保存在当前设备浏览器/Tauri WebView 的 localStorage 中。

设计思路：

- 移动网络不稳定时不丢内容。
- 发送失败原因保存在 `lastError`，便于用户判断是 PC 不在线、端口不通还是 OCR/API 错误。

## 浏览器扩展

实现位置：

```text
apps/guaxi-capture/browser-extension/
  manifest.json
  content.js
  background.js
  popup.html
  popup.js
```

功能：

- 点击扩展按钮发送当前网页。
- 点击扩展按钮发送选中内容。
- 右键页面发送当前网页。
- 右键选区发送选中内容。

提取策略：

1. 优先使用当前选区 HTML。
2. 没有选区时优先找 `article`。
3. 然后尝试 `main`、`[role="main"]`、`.article`、`.content`。
4. 最后 fallback 到 `document.body`。
5. 清理 `script`、`style`、`iframe`、事件属性、内联样式等。

扩展发送到 Capture App 桌面壳本地接收器：

```text
http://127.0.0.1:14230/capture
```

关键日志：

```text
[guaxi-extension]
```

说明：

- 浏览器扩展只适用于 PC 浏览器。
- 移动端网页分享后续应走 Android Share Intent / iOS Share Extension。

## 本地 HTTP 接收器

实现位置：

```text
apps/guaxi-capture/src-tauri/src/lib.rs
```

监听地址：

```text
127.0.0.1:14230
```

路径：

```text
POST /capture
POST /capture/import
OPTIONS *
```

限制：

- 仅绑定 `127.0.0.1`，不暴露到局域网。
- 单次请求最大 2 MB。
- 只接受 JSON payload。

接收后通过 Tauri event 发送给前端：

```text
guaxi-capture-incoming
```

设计思路：

- 浏览器扩展不直接写入 Guaxi 文件库。
- Capture App 收到内容后先生成草稿，让用户确认、修改、补标签，再提交。
- 避免网页误操作直接污染文件库。

关键日志：

```text
[capture-receiver]
```

## 移动端构建方法

### 通用前端构建

```bash
cd apps/guaxi-capture
npm run build
```

### Android 本地构建

Windows 可以构建 Android。需要先安装：

- Android Studio
- Android SDK
- Android NDK
- Android Build Tools
- Platform Tools
- Java / JDK
- Rust Android targets

检查环境：

```powershell
cd apps\guaxi-capture
npm run mobile:check
```

安装 Rust Android targets：

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

设置环境变量示例：

```powershell
$env:ANDROID_HOME="C:\Users\user\AppData\Local\Android\Sdk"
$env:NDK_HOME="C:\Users\user\AppData\Local\Android\Sdk\ndk\<NDK版本>"
```

初始化 Android 工程：

```powershell
cd apps\guaxi-capture
npm run android:init
```

调试：

```powershell
npm run android:dev
```

构建：

```powershell
npm run android:build
```

CI 构建脚本：

```powershell
npm run android:init:ci
npm run android:build:ci
```

常见产物目录：

```text
apps/guaxi-capture/src-tauri/gen/android/app/build/outputs/
```

### iOS 本地构建

iOS 必须使用 macOS + Xcode，Windows 不能直接构建 iOS。

初始化：

```bash
cd apps/guaxi-capture
npm install
npm run ios:init
```

调试：

```bash
npm run ios:dev
```

构建：

```bash
npm run ios:build
```

CI 构建脚本：

```bash
npm run ios:init:ci
npm run ios:build:ci
```

签名、真机安装、TestFlight 和 App Store 发布仍需要 Apple Developer 账号、证书和 provisioning profile。

## GitHub Actions 构建 iOS

workflow：

```text
.github/workflows/guaxi-capture-ios.yml
```

触发方式：

- 修改 `apps/guaxi-capture/**` 后 push。
- 手动 `workflow_dispatch`。

runner：

```text
macos-14
```

主要步骤：

1. Checkout 仓库。
2. 输出 Xcode 版本。
3. 安装 Node.js 22。
4. 安装 Rust stable 和 iOS targets。
5. 缓存 Cargo registry 和 target。
6. `npm ci` 安装依赖。
7. 可选导入 Apple 签名证书和描述文件。
8. 构建前端。
9. 初始化 Tauri iOS 工程。
10. 执行 `npm run ios:build:ci`。
11. 如果开启签名，使用 `xcodebuild -exportArchive` 导出 IPA。
12. 上传 `.ipa`、`.app`、`.xcarchive`、`.dSYM` artifacts。

无签名构建：

- 不需要 Apple secrets。
- 适合验证代码和生成 Xcode 构建产物。
- 通常不能直接安装到真机。

签名 IPA 所需 GitHub Secrets：

| Secret | 说明 |
| --- | --- |
| `APPLE_CERTIFICATE_BASE64` | `.p12` 证书 base64 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 证书密码 |
| `APPLE_PROVISIONING_PROFILE_BASE64` | `.mobileprovision` 描述文件 base64 |
| `KEYCHAIN_PASSWORD` | CI 临时 keychain 密码 |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `APPLE_PROVISIONING_PROFILE_NAME` | 描述文件名称 |

生成 base64 示例：

```bash
base64 -i certificate.p12 | pbcopy
base64 -i GuaxiCapture.mobileprovision | pbcopy
```

手动运行 workflow 时：

- `signed=false`：只做无签名构建。
- `signed=true`：导入证书并尝试导出 IPA。
- `export_method=development`：开发证书真机调试。
- `export_method=ad-hoc`：Ad Hoc 分发。
- `export_method=app-store`：TestFlight / App Store。

## 移动端连接 PC 的网络要求

手机端不能使用 `127.0.0.1` 访问 PC。必须使用 PC 在局域网中的 IP。

示例：

```text
Guaxi 接口：http://192.168.1.100:14219/extension/capture/import
OCR 接口：http://192.168.1.100:9876/ocr
```

要求：

- 手机和 PC 在同一局域网。
- PC 防火墙允许 `14219` 和 `9876` 入站访问。
- Guaxi PC 主程序已经启动后端服务。
- OCR 功能需要 Guaxi PC 的 agent_server 正常运行。

安全建议：

- 目前 Capture Import API 主要面向本机/局域网开发验证。
- 如果后续要公网使用，需要增加认证、设备配对、token、签名或端到端加密。
- OCR 服务也不建议裸露到公网。

## 日志与排错

| 日志前缀 | 来源 | 说明 |
| --- | --- | --- |
| `[capture-intake]` | Capture App 前端 | 分享 payload 解析、Tauri event、启动参数 |
| `[capture-markdown]` | Capture App 前端 | HTML 转 Markdown |
| `[capture-ocr]` | Capture App 前端 / Tauri 壳 | OCR 请求、代理、结果 |
| `[capture-export]` | Capture App 前端 | 复制/导出 Markdown |
| `[capture-receiver]` | Capture App Tauri 壳 | 本地 HTTP 接收器 |
| `[capture-native]` | Capture App Tauri 壳 | 启动参数 |
| `[capture]` | Capture App 前端 | 提交 Guaxi PC |
| `[capture_import]` | Guaxi PC 后端 | Capture Import API |
| `[guaxi-extension]` | 浏览器扩展 | 页面提取和发送 |

常见问题：

### Android 初始化失败：`ANDROID_HOME not set`

安装 Android Studio 和 SDK 后设置：

```powershell
$env:ANDROID_HOME="C:\Users\user\AppData\Local\Android\Sdk"
```

### Android 找不到 NDK

在 Android Studio SDK Manager 中安装 NDK，然后设置：

```powershell
$env:NDK_HOME="C:\Users\user\AppData\Local\Android\Sdk\ndk\<NDK版本>"
```

### Android Dev 一直等待 `192.168.x.x:1430`

现象：

```text
Info Using 192.168.1.100 to access the development server.
Warn Waiting for your frontend dev server to start on http://192.168.1.100:1430/...
```

原因：

- Tauri Android 会让手机或模拟器访问电脑的局域网 IP。
- 如果 Vite 只监听 `127.0.0.1`，它只允许电脑本机访问。
- 手机访问 `192.168.x.x:1430` 时就会连不上。

解决：

- `apps/guaxi-capture/package.json` 的 `dev` 脚本使用 `--host 0.0.0.0`。
- `apps/guaxi-capture/vite.config.js` 使用 `process.env.TAURI_DEV_HOST || '0.0.0.0'`。
- 确认 Windows 防火墙允许 Node/Vite 的 `1430` 端口入站。

验证：

```powershell
cd apps\guaxi-capture
npm run dev
```

输出里应该能看到类似：

```text
Network: http://192.168.1.100:1430/
```

### Gradle Wrapper 下载 `gradle-*.zip` 超时

现象：

```text
Downloading https://services.gradle.org/distributions/gradle-8.14.3-bin.zip
java.io.IOException: Downloading ... failed: timeout
Caused by: java.net.SocketTimeoutException: Read timed out
```

含义：

- 这时 Rust Android 动态库通常已经编译成功。
- 失败点在 Android Gradle Wrapper 下载 Gradle 发行包。
- 这是网络下载问题，不是 Guaxi Capture 代码编译错误。

解决方式一：切换 Gradle Wrapper 下载镜像。

```powershell
cd apps\guaxi-capture
npm run android:gradle:mirror
npm run android:dev
```

脚本会修改：

```text
apps/guaxi-capture/src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties
```

把：

```text
https://services.gradle.org/distributions/gradle-*.zip
```

替换为国内镜像：

```text
https://mirrors.cloud.tencent.com/gradle/gradle-*.zip
```

如果以后要切回官方地址：

```powershell
npm run android:gradle:official
```

说明：

- `android:gradle:mirror` 会通过 `scripts/run-android-gradle-config.mjs` 查找可用 Bash，然后调用 `scripts/configure-android-gradle.sh`。
- 默认会把 Gradle 下载地址改到腾讯云 Gradle 镜像。
- Windows 下建议安装 Git for Windows；如果提示找不到 Bash，可以把 `GUAXI_BASH` 指向你的 `bash.exe`。
- 这个脚本只重写 Android 生成工程里的 `gradle-wrapper.properties`，不会修改 Guaxi 主应用代码。

如果出现：

```text
No value with key 'distributionUrl' specified in wrapper properties file
```

通常是 `gradle-wrapper.properties` 被写入了 UTF-8 BOM，或 `distributionUrl` 行被破坏。继续执行：

```powershell
npm run android:gradle:mirror
```

脚本会用简单的 ASCII properties 内容重写该文件；如果 `distributionUrl` 缺失，会按默认 `gradle-8.14.3-bin.zip` 自动补回。需要指定版本时：

```bash
node scripts/run-android-gradle-config.mjs --gradle-version 8.14.3
```

解决方式二：手动下载 `gradle-8.14.3-bin.zip`，放到 Gradle wrapper cache。这个方式不推荐作为默认流程，因为 cache 路径包含 hash，不如直接替换 `distributionUrl` 稳定。

### iOS 本地没有命令

Windows 上 Tauri CLI 不显示 `ios` 子命令是正常现象。iOS 构建需要 macOS runner 或本地 macOS + Xcode。

### 手机端发送失败

检查：

- App 中 `Guaxi 接口` 是否使用 PC 局域网 IP。
- PC 和手机是否同一网络。
- Windows 防火墙是否允许端口。
- Guaxi PC 主程序是否已启动。
- `POST /extension/capture/import` 是否可访问。

### OCR 失败

检查：

- `OCR 接口` 是否使用 PC 局域网 IP。
- Guaxi PC 主程序的 agent_server 是否已启动。
- `http://电脑IP:9876/ocr` 是否可访问。
- 图片是否已成功读入为 base64。

## 后续演进建议

### Android Share Intent

目标：

- 在 Android 系统分享菜单中显示 Guaxi Capture。
- 支持分享文本、链接、图片。
- 将 intent payload 转换为 `guaxi-capture-incoming` 事件。

建议实现：

- 在 Tauri Android 工程中增加 Intent filter。
- 原生层解析 `ACTION_SEND` / `ACTION_SEND_MULTIPLE`。
- 图片 URI 转 base64 asset。
- 统一投递到前端 `shareIntake.js`。

### iOS Share Extension

目标：

- 在 iOS 分享面板中显示 Guaxi Capture。
- 支持 Safari、微信、知乎、小红书、B站等来源分享。

建议实现：

- 在 Xcode 工程中增加 Share Extension target。
- Extension 将分享 payload 写入 App Group。
- 主 App 启动后读取 App Group 数据并投递给 `shareIntake.js`。

### 移动端本机 OCR

当前 OCR 依赖 Guaxi PC agent_server。后续可选：

- Android 使用 ML Kit Text Recognition。
- iOS 使用 Vision Framework。
- 识别结果仍按 `buildOcrMarkdown()` 追加到草稿。

### 远程同步和分享

当前 App 通过 PC 局域网导入。后续可增加：

- Guaxi Account 登录。
- 设备配对。
- 上传到云端中转。
- 文件库分享链接。
- 权限控制和撤销访问。
