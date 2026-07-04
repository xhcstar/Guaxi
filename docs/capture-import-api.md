# Guaxi Capture Import API

Guaxi 主程序提供统一采集入口，供浏览器插件、未来的手机端 Tauri App、OCR 工具或其他自动化入口把内容导入文件库。

## Endpoint

`POST http://127.0.0.1:<guaxi-service-port>/extension/capture/import`

该接口假设调用方已经把网页、截图 OCR、笔记或平台分享内容转换成 Markdown。服务端负责：

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
- 正文可使用 `markdown`、`content` 或 `text`
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
- OCR 可以先在手机端完成，结果填入 `markdown`。
- 原图或截图放入 `assets`，服务端会落到文档的 `images` 目录。
- 离线时把请求存入本地队列，联网后重试。

## Example

```bash
curl -X POST "http://127.0.0.1:14219/extension/capture/import" \
  -H "Content-Type: application/json" \
  -d "{\"title\":\"Demo\",\"sourceApp\":\"camera\",\"knowledgeBase\":\"AI\",\"fileRepo\":\"Inbox\",\"markdown\":\"# Demo\\n\\nOCR text\"}"
```

