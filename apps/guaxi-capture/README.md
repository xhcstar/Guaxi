# Guaxi Capture

独立的 Guaxi 采集端 App。代码放在 `apps/guaxi-capture`，不与 Guaxi 主程序的 `src`、`src-tauri` 混放。

## 目标

- 接收手动笔记、网页分享内容、系统分享文本/链接和图片附件。
- 通过 Guaxi 主程序的 `POST /extension/capture/import` 统一导入文件库。
- 离线或连接失败时进入本地队列，稍后重试。
- 图片、截图和拍照内容进入 PC OCR 队列，由 Guaxi PC 端处理。
- 提供浏览器扩展，把知乎、微信公众号、小红书、B站等网页正文发送到 Capture App。

## 运行

```bash
cd apps/guaxi-capture
npm run dev
```

Tauri 桌面壳：

```bash
cd apps/guaxi-capture
npm run tauri:dev
```

## Android 构建

Android 可以在 Windows 上构建，但需要先安装：

- Android Studio
- Android SDK
- Android NDK
- Android Build Tools
- Java / JDK
- Rust Android targets

首次初始化 Android 工程：

```bash
cd apps/guaxi-capture
npm run mobile:check
npm run android:init
npm run android:share:patch
```

连接真机或启动模拟器后调试：

```bash
cd apps/guaxi-capture
npm run android:dev
```

构建 Android 安装包：

```bash
cd apps/guaxi-capture
npm run android:build
```

构建产物通常会在：

```text
apps/guaxi-capture/src-tauri/gen/android/app/build/outputs/
```

## iOS 构建

iOS 不能在 Windows 上直接构建，必须在 macOS 上使用 Xcode。把仓库放到 macOS 后执行：

```bash
cd apps/guaxi-capture
npm install
npm run ios:init
npm run ios:dev
```

构建 iOS：

```bash
cd apps/guaxi-capture
npm run ios:build
```

然后在 Xcode 中签名、选择 Team，并归档发布到 TestFlight 或导出 IPA。

## GitHub Actions 构建 iOS

仓库已提供 GitHub Actions 配置：

```text
.github/workflows/guaxi-capture-ios.yml
```

使用方式：

1. 把当前代码推送到 GitHub。
2. 打开 GitHub 仓库的 `Actions` 页面。
3. 选择 `Build Guaxi Capture iOS`。
4. 点击 `Run workflow`。
5. 构建完成后，在 workflow run 的 `Artifacts` 中下载 `guaxi-capture-ios-*`。

### 无签名构建

不配置 Apple secrets 时，workflow 会尝试在 GitHub 的 macOS runner 上初始化并构建 iOS 工程。这个模式适合验证代码和生成 Xcode 构建产物，但通常不能直接安装到真机。

### 签名 IPA

如果你要得到可安装到真机、Ad Hoc 或提交 TestFlight/App Store 的 IPA，需要 Apple Developer 账号，并在 GitHub 仓库 `Settings -> Secrets and variables -> Actions` 添加这些 secrets：

- `APPLE_CERTIFICATE_BASE64`：`.p12` 证书的 base64 内容
- `APPLE_CERTIFICATE_PASSWORD`：`.p12` 证书密码
- `APPLE_PROVISIONING_PROFILE_BASE64`：`.mobileprovision` 描述文件的 base64 内容
- `KEYCHAIN_PASSWORD`：CI 临时 keychain 密码，自己生成一个强密码即可
- `APPLE_TEAM_ID`：Apple Developer Team ID
- `APPLE_PROVISIONING_PROFILE_NAME`：描述文件名称

在 macOS 上生成 base64 的示例：

```bash
base64 -i certificate.p12 | pbcopy
base64 -i GuaxiCapture.mobileprovision | pbcopy
```

然后在 `Run workflow` 时把 `signed` 设为 `true`，并根据证书类型选择 `development`、`ad-hoc` 或 `app-store`。

## 移动端连接 Guaxi 主程序

手机上的 `127.0.0.1` 指手机本机，不是运行 Guaxi 主程序的电脑。因此 Android/iOS 真机测试时，需要把界面里的接口改成电脑局域网 IP：

```text
Guaxi 接口：http://你的电脑IP:14219/extension/capture/import
```

电脑和手机需要在同一局域网，Windows 防火墙要允许 `14219` 被局域网访问。OCR 不需要从手机直连，后续由 PC 端内部任务处理。

## 当前边界

当前版本已完成收件箱式采集队列、附件编码、离线队列、外部分享接收层、Android Share Intent 生成工程补丁、浏览器扩展、本地接收器和 Guaxi Capture API 对接。iOS Share Extension 会在后续原生移动层继续补。

## 外部分享接收

Capture App 会把 URL 参数、剪贴板文本和 Tauri 原生事件统一转换为采集草稿。这样浏览器扩展、移动端 Share Intent 和桌面深链后续只要投递同一类字段即可。

支持的常用字段：

- `title`：标题
- `sourceUrl` / `url` / `link`：来源链接
- `sourceApp` / `source` / `from`：来源平台，例如 `wechat`、`zhihu`、`xiaohongshu`、`bilibili`
- `markdown` / `content` / `text`：Markdown 或正文
- `html` / `articleHtml` / `selectionHtml`：文章 HTML，Capture App 会自动转成 Markdown
- `knowledgeBase` / `kb`：知识库
- `fileRepo` / `repo`：文件库
- `tags`：逗号分隔标签

调试示例：

```text
http://127.0.0.1:1430/?title=Demo&sourceApp=zhihu&sourceUrl=https%3A%2F%2Fwww.zhihu.com%2Fquestion%2F1&markdown=%23%20Demo%0A%0Ahello
```

HTML 转 Markdown 调试示例：

```text
http://127.0.0.1:1430/?title=HTML%20Demo&sourceApp=wechat&html=%3Ch1%3E标题%3C%2Fh1%3E%3Cp%3E正文%3Cstrong%3E重点%3C%2Fstrong%3E%3C%2Fp%3E
```

Tauri 原生层后续可以向前端发送 `guaxi-capture-incoming` 事件，事件 payload 使用同样的字段结构。

桌面端也可以用启动参数模拟分享入口：

```bash
npm run tauri:dev -- -- --capture-json "{\"title\":\"Demo\",\"sourceUrl\":\"https://example.com\",\"markdown\":\"# Demo\"}"
npm run tauri:dev -- -- --capture-text "# Demo"
npm run tauri:dev -- -- --capture-url "https://example.com"
```

Rust 启动层会打印 `[capture-native]` 日志，前端接收层会打印 `[capture-intake]` 日志，便于定位分享链路是否进入 App。

## 本地接收器

Tauri 桌面壳启动后会监听本机地址：

```text
http://127.0.0.1:14230/capture
http://127.0.0.1:14230/capture/import
```

浏览器插件可以把文章内容 POST 到这里，Capture App 会通过 `guaxi-capture-incoming` 事件把 payload 转成当前草稿，用户确认后再发送到 Guaxi 文件库。

示例：

```bash
curl -X POST http://127.0.0.1:14230/capture ^
  -H "Content-Type: application/json" ^
  -d "{\"title\":\"Demo\",\"sourceApp\":\"wechat\",\"html\":\"<h1>标题</h1><p>正文</p>\"}"
```

接收器只绑定 `127.0.0.1`，并限制单次请求最大 2MB。Rust 侧会打印 `[capture-receiver]` 日志。

## 图片 OCR 队列

Capture App 不在手机端直接 OCR。选择图片或从 Android 系统分享图片时，App 会把图片转成 base64 asset 放入本地队列；发送到 PC 后，Guaxi PC 端会先生成待处理 Markdown，并把图片落到文档的 `images` 目录。

前端会打印 `[capture-inbox]` 和 `[capture-sync]` 日志，PC 端会打印 `[capture_import]` 日志。后续 PC 端可以基于已入库图片继续执行 OCR 并回写 Markdown。

## Markdown 转换

当前内置轻量 HTML 转 Markdown 转换器，支持标题、段落、链接、图片、引用、代码块、列表和表格。它的定位是先保证采集链路可用、内容可编辑；后续如果要对复杂网页做更高保真提取，可以把 `src/services/htmlToMarkdown.js` 替换为 Readability + Turndown 这类更完整的转换管线。

## 浏览器扩展

扩展目录：

```text
apps/guaxi-capture/browser-extension
```

Chrome / Edge 加载方式：

1. 打开扩展管理页。
2. 开启开发者模式。
3. 选择“加载已解压的扩展程序”。
4. 选择 `apps/guaxi-capture/browser-extension`。

使用方式：

- 点击扩展按钮，选择“发送当前网页”或“发送选中内容”。
- 或右键页面/选区，选择发送到 Guaxi Capture。

扩展会把 HTML 投递到 `http://127.0.0.1:14230/capture`，Capture App 收到后生成草稿。浏览器控制台会打印 `[guaxi-extension]` 日志。

## 构建验证

建议按这个顺序验证：

```bash
cd apps/guaxi-capture
npm run build
cd ../..
cargo test --manifest-path apps/guaxi-capture/src-tauri/Cargo.toml --quiet
cargo check --manifest-path apps/guaxi-capture/src-tauri/Cargo.toml --quiet
```

完整功能验证：

1. 启动 Guaxi 主程序，确认主程序后端和 OCR agent_server 可用。
2. 启动 Capture App：`cd apps/guaxi-capture && npm run tauri:dev`。
3. 在 Capture App 里选择图片点 `OCR图片`，确认 Markdown 区域追加识别文本。
4. 加载 `browser-extension`，打开网页后点击“发送当前网页”，确认 Capture App 自动生成草稿。
5. 点击 `发送到 Guaxi`，确认主程序文件库出现新文档。
