# Guaxi Capture Import API

Guaxi 主程序提供统一采集入口，供浏览器插件、未来的手机端 Tauri App、OCR 工具或其他自动化入口把内容导入文件库。

## Endpoint

PC 本机调用：

`POST http://127.0.0.1:14219/extension/capture/import`

手机或局域网设备调用：

`POST http://<PC-LAN-IP>:14219/extension/capture/import`

设计说明：

- Guaxi 主后端仍保持 `127.0.0.1:8888`，不对局域网暴露完整内部 API。
- Capture Import 单独监听 `0.0.0.0:14219`，只用于移动端/局域网采集写入。
- 可通过环境变量 `GUAXI_CAPTURE_BIND` 覆盖监听地址，例如 `127.0.0.1:14219` 或 `0.0.0.0:14219`。

该接口支持两种输入模式：

- 已转换内容：调用方提供 `markdown`、`content` 或 `text`，PC 端直接写入文件库。
- PC 端待处理内容：调用方只提供 `sourceUrl` 或 `assets`，PC 端先生成待处理 Markdown，并把原始图片/附件落库，后续由 PC 端执行网页转换、OCR 或人工整理。

服务端负责：

- 写入 `collect_capture` 暂存 Markdown。
- 导入到 `Libraries/{fileRepo}/{document}/md`。
- 可选附件写入 `Libraries/{fileRepo}/{document}/images`。
- 写入 `imported_files` 数据库。
- 发送 `repo_imported_files_changed` 事件刷新 Repo 页面。

## Request

```json
{
  "title": "文章标题",
  "sourceUrl": "https://example.com/article",
  "sourceApp": "wechat",
  "description": "摘要或备注",
  "knowledgeBase": "AI",
  "fileRepo": "Inbox",
  "tags": ["capture", "wechat"],
  "createdAt": "2026-07-04 18:30:00",
  "markdown": "# 文章标题\n\n正文内容...",
  "assets": [
    {
      "name": "photo-001.jpg",
      "mime": "image/jpeg",
      "dataBase64": "/9j/4AAQSkZJRgABAQ..."
    }
  ]
}
```

兼容字段：

- `knowledge_repo` 或 `knowledgeBase`
- `file_repo` 或 `fileRepo`
- `source_url` 或 `sourceUrl`
- `source_app` 或 `sourceApp`
- `created_at`、`createdAt` 或 `timestamp`
- 正文可使用 `markdown`、`content` 或 `text`；如果提供了 `sourceUrl` 或 `assets`，正文可以为空
- 附件 base64 可使用 `data_base64` 或 `dataBase64`

## Response

```json
{
  "success": true,
  "message": "Capture imported successfully",
  "file_id": 21,
  "source_path": "D:\\MyProject\\Guaxi\\TestRepo\\Libraries\\Inbox\\capture_xxx\\md\\capture_xxx.md",
  "library": "Inbox",
  "knowledge_repo": "AI",
  "asset_count": 1
}
```

## Mobile App Notes

手机端 Tauri App 建议先做采集端，不直接写 Guaxi 的 SQLite 或文件目录：

- Android 使用 Share Intent 接收 URL、选中文本、图片。
- iOS 使用 Share Extension 接收 URL、文本、图片。
- 手机端不需要承担 OCR 和复杂网页转换。图片、截图或拍照内容放入 `assets` 后，PC 端会先作为 OCR 队列文档入库。
- 分享链接可以只填 `sourceUrl`，PC 端后续负责抓取、转换或人工确认。
- 离线时把请求存入本地队列，联网后重试。

## Example

```bash
curl --noproxy "*" -X POST "http://127.0.0.1:14219/extension/capture/import" \
  -H "Content-Type: application/json" \
  -d "{\"title\":\"Demo\",\"sourceApp\":\"camera\",\"knowledgeBase\":\"AI\",\"fileRepo\":\"Inbox\",\"markdown\":\"# Demo\\n\\nOCR text\"}"
```

局域网测试：

```bash
curl --noproxy "*" -X POST "http://192.168.1.100:14219/extension/capture/import" \
  -H "Content-Type: application/json" \
  -d "{\"title\":\"Mobile Demo\",\"sourceApp\":\"camera\",\"knowledgeBase\":\"AI\",\"fileRepo\":\"Inbox\",\"markdown\":\"# Mobile Demo\\n\\nfrom phone\"}"
```

图片进入 PC OCR 队列示例：

```bash
curl --noproxy "*" -X POST "http://192.168.1.100:14219/extension/capture/import" \
  -H "Content-Type: application/json" \
  -d "{\"title\":\"Photo OCR Queue\",\"sourceApp\":\"camera\",\"knowledgeBase\":\"AI\",\"fileRepo\":\"Inbox\",\"tags\":[\"capture\",\"ocr\",\"camera\"],\"assets\":[{\"name\":\"photo.jpg\",\"mime\":\"image/jpeg\",\"dataBase64\":\"...\"}]}"
```

注意：如果命令行环境设置了 `http_proxy`，普通 `curl http://192.168.x.x:14219/...` 会先走代理，可能得到代理返回的 `502 Bad Gateway`，不能代表 Guaxi PC 服务不可用。测试局域网地址时使用 `--noproxy "*"`。
