# Guaxi Capture 移动端 App 构建与交互设计

## 目标

Guaxi Capture 是一个独立于 Guaxi PC 主程序的采集 App，代码位于 `apps/guaxi-capture`。它的目标是把手机端、浏览器端、系统分享、剪贴板、拍照图片等来源统一收进移动端采集队列；当 App 与 Guaxi PC 端互联后，再通过 Capture Import API 发送给 PC，由 PC 端执行网页转换、OCR 和文件库入库。

设计重点：

- App 代码独立，不和 Guaxi PC 主程序的 `src`、`src-tauri` 混放。
- 移动端优先支持 Android 和 iOS 构建。
- Guaxi PC 端作为文件库、OCR 服务和导入服务的承载端。
- Capture App 只负责采集、预览、编辑、离线队列和提交；复杂转换、OCR 和最终入库都放在 PC 端。
- 所有入口统一归一化为同一份草稿结构，避免每个平台各写一套转换逻辑。

## 目录结构

```text
apps/guaxi-capture/
  browser-extension/              # Chrome / Edge 扩展，用于网页采集
  scripts/check-mobile-env.ps1    # Windows Android 构建环境检查
  scripts/configure-android-share-target.mjs # Android 系统分享入口补丁
  src/
    main.js                       # Capture App UI 和交互逻辑
    styles.css                    # Capture App 样式
    services/
      captureClient.js            # 发送到 Guaxi PC Capture Import API
      htmlToMarkdown.js           # 轻量 HTML -> Markdown 转换，主要用于桌面/调试入口
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

- 收件箱式采集队列：待发送、失败、已发送、草稿。
- Android 系统分享入口：支持文本、链接、单图、多图分享到 Guaxi Capture。
- 手动创建笔记草稿。
- 从 URL 参数生成草稿。
- 从剪贴板读取文本或 HTML。
- 接收 Tauri 原生事件 `guaxi-capture-incoming`。
- 桌面壳支持启动参数 `--capture-json`、`--capture-text`、`--capture-url`。
- 桌面壳支持本地 HTTP 接收器 `127.0.0.1:14230/capture`。
- 浏览器扩展提取当前网页或选中内容，并发送到 Capture App。
- 图片附件读取为 base64。
- 图片、截图、拍照内容进入 PC OCR 队列，由 PC 端处理。
- 离线队列，发送失败时保留内容，稍后重试。
- 发送到 Guaxi PC 文件库。

后续移动端可继续补充：

- iOS Share Extension。
- 移动端原生相机入口。
- 用户登录、远程同步、端到端分享权限。

## 交互架构

```text
知乎 / 微信公众号 / 小红书 / B站 / 浏览器选区
            |
            | browser-extension / Share Intent / Clipboard / URL params
            v
Guaxi Capture App
  shareIntake.js        # 归一化 payload
  htmlToMarkdown.js     # 桌面/调试入口的 HTML -> Markdown
  captureClient.js      # 提交 Guaxi PC
            |
            | POST /extension/capture/import
            v
Guaxi PC 后端
  chrome_extension_service.rs
  PC-side conversion / OCR
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

## 移动端 UI 与使用流程

当前界面采用“连接状态 + 采集队列 + 详情编辑”的结构：

1. 顶部显示 PC 连接地址、待发送数量、失败数量和最近状态。
2. 快捷入口提供“剪贴板”“图片 OCR 队列”“手记”三类高频动作。
3. 左侧/上方列表按“全部、待发送、失败、已发送”筛选采集项。
4. 右侧/下方详情面板编辑标题、来源、库、标签和 Markdown 备注。
5. “同步队列”只发送 `pending` 和 `failed` 项；空草稿会留在本地，编辑后自动变成待发送。

设计思路：

- 手机端优先解决“快速收进来”和“不会丢”的问题，不把复杂整理工作压在小屏幕上。
- 分享链接、截图、拍照图片可以先进入队列；PC 在线后统一发送。
- 发送失败保留 `lastError`，便于区分网络、端口、防火墙或 PC 服务问题。

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

## 图片 OCR 队列

实现位置：

```text
apps/guaxi-capture/src/main.js
apps/guaxi-capture/src/services/captureClient.js
src-tauri/src/guaxiservices/chrome_extension_service.rs
```

当前设计中，移动端不直接执行 OCR。图片、截图或拍照内容会被读取成 base64 asset，进入本地采集队列；当队列发送到 Guaxi PC 端后，PC 端先生成一份待处理 Markdown，并把图片落到文档的 `images` 目录。

PC 主程序已有 OCR command：

```text
src-tauri/src/commands/ocr.rs
commands::ocr::ocr_recognize
```

后续可以在 PC 端基于已入库的 `images` 资产增加后台 OCR 任务，把识别结果回写到对应 Markdown 文档。

设计思路：

- 手机端只做事实采集，不消耗移动端算力和电量。
- OCR 服务、文件库路径、数据库写入都集中在 PC 端，失败恢复和日志更容易统一。
- 即使 PC 暂时没有 OCR 任务，原图也已经安全进入文件库，不会丢失。

关键日志：

```text
[capture-inbox]
[capture_import]
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

PC 端设计：

- Guaxi PC 主后端仍监听 `127.0.0.1:8888`，不暴露完整内部 API。
- Capture Import 额外监听 `0.0.0.0:14219`，只接收 `POST /extension/capture/import`。
- 可以用 `GUAXI_CAPTURE_BIND` 覆盖监听地址；例如仅本机调试时设为 `127.0.0.1:14219`。

移动端默认值：

- Android dev 下，`scripts/run-vite-dev.mjs` 会把 `VITE_GUAXI_PC_HOST` 设置为电脑局域网 IP。
- App 首次打开时默认 `Guaxi 接口` 会变成 `http://电脑IP:14219/extension/capture/import`。
- 旧版本保存过的 `127.0.0.1:14219` 会在移动端自动忽略，避免手机把请求发给自己。

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
- Android 移动端网页分享走系统 Share Intent；iOS 仍需后续补 Share Extension。

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

启用 Android 系统分享入口：

```powershell
npm run android:share:patch
```

通常不需要手动执行这一步：`android:init` 完成后会自动运行，`android:dev`、`android:build` 和 `android:dev:fresh` 前也会尝试自动补丁。手动命令主要用于排障或确认生成工程已经写入分享入口。

该脚本会修改 Tauri 生成目录 `src-tauri/gen/android`：

- 给 `MainActivity` 增加 `ACTION_SEND text/*`、`ACTION_SEND image/*`、`ACTION_SEND_MULTIPLE image/*`。
- 写入 `ShareIntentPlugin.kt`，把 Android 分享 intent 转成前端可读取的 payload。
- 注册 Tauri plugin command `plugin:guaxi-share-intent|readPendingShare`，前端会轮询并进入采集队列。

每次删除或重新生成 `src-tauri/gen/android` 后，重新执行 `npm run android:init` 或 `npm run android:share:patch`。

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
```

要求：

- 手机和 PC 在同一局域网。
- PC 防火墙允许 `14219` 入站访问。
- 命令行测试时绕过代理：`curl --noproxy "*"`。
- Guaxi PC 主程序已经启动后端服务。
- OCR 由 PC 端后续任务处理；手机端不需要直接访问 OCR 端口。

安全建议：

- 目前 Capture Import API 主要面向本机/局域网开发验证。
- 如果后续要公网使用，需要增加认证、设备配对、token、签名或端到端加密。
- OCR 服务不建议裸露到公网，应尽量保持为 PC 本机内部服务。

## 日志与排错

| 日志前缀 | 来源 | 说明 |
| --- | --- | --- |
| `[capture-intake]` | Capture App 前端 | 分享 payload 解析、Tauri event、启动参数 |
| `[capture-inbox]` | Capture App 前端 | 队列创建、图片进入 OCR 队列、状态更新 |
| `[capture-sync]` | Capture App 前端 | 队列发送成功或失败 |
| `[capture-markdown]` | Capture App 前端 | HTML 转 Markdown |
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

### ADB 安装 APK 失败：`Failure [-99]`

现象：

```text
Performing Streamed Install
adb.exe: failed to install ... app-arm64-debug.apk: Failure [-99]
failed to run Android app
```

含义：

- 这时 Gradle 下载、Rust 编译、Android APK 构建都已经完成。
- 失败点在 `adb install` 把 APK 安装到手机的阶段。
- 常见原因是手机上已有同包名 `com.guaxi.capture` 的旧安装、签名不一致、用户空间里残留安装状态，或设备端安装会话被系统拒绝。

先做设备诊断：

```powershell
cd apps\guaxi-capture
npm run android:adb:devices
npm run android:adb:diagnose
```

清理旧包后重新启动 dev：

```powershell
npm run android:dev:fresh
```

如果连接了多台设备，先指定序列号：

```powershell
$env:GUAXI_ANDROID_SERIAL="e009af14"
npm run android:dev:fresh
```

设计说明：

- `android:dev:fresh` 会先执行 `adb uninstall com.guaxi.capture` 和 `pm uninstall --user 0 com.guaxi.capture`，再运行 Tauri Android dev。
- 这只清理 Guaxi Capture 手机端包名，不会影响 Guaxi PC 端代码。
- 如果仍然失败，打开手机屏幕确认是否有安装确认弹窗、USB 调试授权弹窗，或系统安全软件拦截安装。

### Android 安装成功但打开后空白

现象：

```text
Performing Streamed Install
Success
Starting: Intent { cmp=com.guaxi.capture/.MainActivity }
Tauri/Console: File: http://tauri.localhost/src/services/shareIntake.js - Line ... - Msg: Uncaught TypeError
```

含义：

- APK 已经安装并启动成功。
- 空白页来自 Android WebView 内部的前端 JS 异常，或者 dev 模式下 WebSocket/HMR 地址仍指向 `localhost`。
- 手机里的 `localhost` 是手机自己，不是电脑，所以 dev server 和 HMR 必须使用电脑的局域网 IP。

当前处理：

- `shareIntake` 已经对 `null`、非对象 payload 做防御，首屏没有分享参数时不会再抛异常。
- `npm run dev` 会通过 `scripts/run-vite-dev.mjs` 自动探测局域网 IP，并设置 `VITE_HMR_HOST`。
- Tauri CSP 已允许 dev 模式访问 `http/ws`，避免 WebView 拦截局域网接口和 HMR。

重新运行：

```powershell
cd apps\guaxi-capture
npm run android:dev:fresh
```

如果仍然白屏，重新贴 `Tauri/Console:` 后面的 JS 错误行，优先看 `Uncaught ...`，不要只看前面的 Android 系统警告。

### iOS 本地没有命令

Windows 上 Tauri CLI 不显示 `ios` 子命令是正常现象。iOS 构建需要 macOS runner 或本地 macOS + Xcode。

### 手机端发送失败

检查：

- App 中 `Guaxi 接口` 是否使用 PC 局域网 IP。
- PC 和手机是否同一网络。
- Windows 防火墙是否允许端口。
- Guaxi PC 主程序是否已启动。
- `POST /extension/capture/import` 是否可访问。

### 图片没有进入 OCR 队列

检查：

- App 中是否选择了图片，或者 Android 分享来源是否确实传入了图片流。
- 图片是否已成功读入为 base64。
- Guaxi PC 主程序是否已启动，`POST /extension/capture/import` 是否可访问。
- PC 端后续 OCR 任务是否正常运行。

### Android 分享能看到 Guaxi，但收件箱没有记录

先重新给生成工程打分享补丁并重装：

```powershell
cd apps\guaxi-capture
npm run android:share:patch
npm run android:dev:fresh
```

检查点：

- `AndroidManifest.xml` 里是否存在 `ACTION_SEND text/*`、`ACTION_SEND image/*`、`ACTION_SEND_MULTIPLE image/*`。
- `MainActivity.kt` 是否注册了 `ShareIntentPlugin`，并覆盖了 `onNewIntent(intent: Intent)`。
- `ShareIntentPlugin.kt` 是否存在，日志里是否能看到 `GuaxiShareIntent`。
- Android WebView 控制台是否有 `[capture-intake] android share payload loaded` 或 `[capture-intake] android share dom event received`。
- 如果看到 `event.listen not allowed`，确认 `src-tauri/capabilities/default.json` 中包含 `core:event:allow-listen`，然后重装 App。

设计说明：

- 从知乎、公众号等 App 分享链接时，移动端只生成带 `sourceUrl` 的待发送记录。
- 该记录不会立刻把分享文本当 Markdown 入库；PC 端收到后再根据 URL 抓取和转换正文。
- 收到分享后默认停留在收件箱，用户确认连接 PC 后再点“同步”。

## 后续演进建议

### Android Share Intent

当前已提供生成工程补丁脚本：

```powershell
cd apps\guaxi-capture
npm run android:init
npm run android:share:patch
npm run android:dev:fresh
```

目标：

- 在 Android 系统分享菜单中显示 Guaxi Capture。
- 支持分享文本、链接、图片。
- 将 intent payload 转换为前端采集队列。

实现说明：

- 在 Tauri Android 生成工程中增加 Intent filter。
- 原生 `ShareIntentPlugin` 解析 `ACTION_SEND` / `ACTION_SEND_MULTIPLE`。
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

### 远程同步和分享

当前 App 通过 PC 局域网导入。后续可增加：

- Guaxi Account 登录。
- 设备配对。
- 上传到云端中转。
- 文件库分享链接。
- 权限控制和撤销访问。
