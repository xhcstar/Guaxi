# Notebook Data Lab 外部调用示例

## 设计说明

这组接口的目标是把“数据实验室”的入口统一成一条链路：

1. 外部程序不直接修改前端状态，而是向 Guaxi 发送标准动作请求。
2. Tauri `command`、HTTP API、以及界面按钮最终都会复用同一套 Notebook Data Lab 事件分发逻辑。
3. 这样后续无论你是接脚本、其他桌面工具，还是自动化工作流，扩展成本都会更低。

当前支持的动作：

- `import-file`
- `import-clipboard`
- `import-inline-data`
- `materialize-variable`
- `writeback-variable`
- `export-variable`

## 接口总览

### Tauri IPC

- 命令名：`notebook_data_lab_rpc`
- 返回：是否成功分发到 Notebook Data Lab 事件桥

### HTTP API

- 健康检查：`GET http://127.0.0.1:8888/api/notebook/data-lab/health`
- 动作入口：`POST http://127.0.0.1:8888/api/notebook/data-lab/action`

## 请求字段

以下字段都使用 `camelCase`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `action` | `string` | 必填，动作名 |
| `filePath` | `string` | 目标笔记路径，用来路由到正确的 Notebook |
| `cellId` | `string` | Python Cell 的运行时 id，可选 |
| `cellKey` | `string` | Python Cell key，可选 |
| `stableId` | `string` | Python Cell 稳定 id，推荐优先传 |
| `variableName` | `string` | 变量名，物化/写回/导出时常用 |
| `dataKind` | `string` | `table` / `structured` / `text` / `value` |
| `sourceType` | `string` | `file` / `clipboard` / `inline` 等 |
| `sourceLabel` | `string` | 来源显示名 |
| `text` | `string` | 内联导入文本 |
| `clientId` | `string` | 调用方 id，便于日志追踪 |

## curl 示例

### 1. 导入外部文件

```bash
curl -X POST "http://127.0.0.1:8888/api/notebook/data-lab/action" \
  -H "Content-Type: application/json" \
  -d '{
    "action": "import-file",
    "filePath": "E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
    "stableId": "py-rs-44dd7ef0-1704",
    "clientId": "external-curl-demo"
  }'
```

### 2. 导入内联 JSON 数据

```bash
curl -X POST "http://127.0.0.1:8888/api/notebook/data-lab/action" \
  -H "Content-Type: application/json" \
  -d '{
    "action": "import-inline-data",
    "filePath": "E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
    "stableId": "py-rs-44dd7ef0-1704",
    "sourceType": "inline",
    "sourceLabel": "sales-report.json",
    "variableName": "sales_report",
    "text": "[{\"month\":\"2026-01\",\"revenue\":1200},{\"month\":\"2026-02\",\"revenue\":1560}]",
    "clientId": "external-curl-demo"
  }'
```

### 3. 将变量生成输出并写回 Markdown

```bash
curl -X POST "http://127.0.0.1:8888/api/notebook/data-lab/action" \
  -H "Content-Type: application/json" \
  -d '{
    "action": "writeback-variable",
    "filePath": "E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
    "stableId": "py-rs-44dd7ef0-1704",
    "variableName": "sales_report",
    "dataKind": "table",
    "clientId": "external-curl-demo"
  }'
```

### 4. 健康检查

```bash
curl "http://127.0.0.1:8888/api/notebook/data-lab/health"
```

## Python 示例

```python
import requests

BASE_URL = "http://127.0.0.1:8888"

payload = {
    "action": "import-inline-data",
    "filePath": r"E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
    "stableId": "py-rs-44dd7ef0-1704",
    "sourceType": "inline",
    "sourceLabel": "lab-data.json",
    "variableName": "lab_data",
    "text": '[{"x": 1, "y": 3}, {"x": 2, "y": 5}]',
    "clientId": "python-requests-demo",
}

response = requests.post(
    f"{BASE_URL}/api/notebook/data-lab/action",
    json=payload,
    timeout=15,
)
response.raise_for_status()
print(response.json())
```

写回或导出时，只需要切换动作名并补上变量信息：

```python
requests.post(
    f"{BASE_URL}/api/notebook/data-lab/action",
    json={
        "action": "export-variable",
        "filePath": r"E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
        "stableId": "py-rs-44dd7ef0-1704",
        "variableName": "lab_data",
        "dataKind": "table",
        "clientId": "python-requests-demo",
    },
    timeout=15,
).raise_for_status()
```

## 前端 `invoke` 示例

### 1. 通过 Tauri Command 导入文件

```js
import { invoke } from '@tauri-apps/api/core'

await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'import-file',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    clientId: 'frontend-demo',
  },
})
```

### 2. 导入内联文本

```js
await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'import-inline-data',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    sourceType: 'inline',
    sourceLabel: 'notes.txt',
    variableName: 'notes_text',
    text: '第一行\\n第二行\\n第三行',
    clientId: 'frontend-demo',
  },
})
```

### 3. 生成输出 / 写回 / 导出

```js
await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'materialize-variable',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    variableName: 'sales_report',
    dataKind: 'table',
    clientId: 'frontend-demo',
  },
})

await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'writeback-variable',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    variableName: 'sales_report',
    dataKind: 'table',
    clientId: 'frontend-demo',
  },
})

await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'export-variable',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    variableName: 'sales_report',
    dataKind: 'table',
    clientId: 'frontend-demo',
  },
})
```

## 返回示例

成功时会返回类似结构：

```json
{
  "success": true,
  "requestId": "lab-rpc-xxxx",
  "delivered": true,
  "eventName": "notebook-data-lab-rpc",
  "action": "import-inline-data",
  "filePath": "E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
  "message": "数据实验室动作 import-inline-data 已派发"
}
```

## 使用建议

1. 优先传 `filePath + stableId`，这样 Notebook 切换、Cell 重排后仍然更稳。
2. 如果是自动化脚本，建议始终带上 `clientId`，后续排查日志会轻松很多。
3. `import-inline-data` 很适合对接其他程序的结构化结果，比如爬虫、数据库查询、LLM 推理结果、第三方分析工具输出。
