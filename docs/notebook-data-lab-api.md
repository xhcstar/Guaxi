# Notebook Data Lab 外部调用示例

## 设计说明

这组接口的目标是把“数据实验室”的入口统一成一条链路：

1. 外部程序不直接修改前端状态，而是向 Guaxi 发送标准动作请求。
2. Tauri `command`、HTTP API、以及界面按钮最终都会复用同一套 Notebook Data Lab 事件分发逻辑。
3. 这样后续无论你是接脚本、其他桌面工具，还是自动化工作流，扩展成本都会更低。

当前支持的动作：

- `import-source` ⭐ 统一导入入口（推荐，覆盖文件 / 数据库 / API / 剪贴板 / 自定义脚本）
- `import-file`
- `import-clipboard`
- `import-inline-data`
- `materialize-variable`
- `writeback-variable`
- `export-variable`

## 统一数据导入抽象（Import Spec / Dataset Descriptor）

`import-source` 用一份可序列化的 **Import Spec** 描述“从哪取数 + 怎么解析”，覆盖三类数据：

- **标准结构化**：`csv` / `tsv` / `excel` / `parquet` / `feather` / `database`（SQLite/Postgres/MySQL/DuckDB）
- **半结构化**：`json` / `jsonl` / `api-json` / `api-csv` / `api-text` / `logline`（正则逐行解析日志）
- **非标准 / 私有**：`custom`（用户提供 `def load(source)` 内联片段或外部 `.py` 文件，处理私有协议 / 二进制 / 混合文本）

### Import Spec 字段

```jsonc
{
  "category": "file | database | api | clipboard | script",
  "format":   "csv|tsv|excel|parquet|feather|database|json|jsonl|api-json|api-csv|api-text|logline|custom|auto",
  "variableName": "目标会话变量名（留空按来源自动命名）",
  "sourceLabel":  "来源显示名",
  "source": {
    "path": "本地文件路径",
    "url": "HTTP URL",
    "text": "内联文本（剪贴板/日志/JSON）",
    "connStr": "数据库连接串，如 sqlite:///x.db、postgresql+psycopg2://user:pwd@host/db",
    "table": "表名", "query": "自定义 SQL",
    "method": "GET|POST…", "headers": { "K": "V" }, "body": "请求体", "auth": "Bearer Token", "jsonPath": "data.items",
    "scriptText": "def load(source): ...", "scriptPath": "/path/to/loader.py", "entry": "load"
  },
  "options": {
    "encoding": "utf-8", "sep": ",", "header": "infer | none",
    "sheet": "工作表名或序号", "jsonPath": "data.items",
    "logPattern": "(?P<ts>\\S+) (?P<level>\\w+) (?P<msg>.*)"
  }
}
```

> `format: "auto"` 会让 Python 运行时按文件扩展名 / URL / 内容自动推断；旧的 `import-file` /
> `import-url` / `import-clipboard` 即等价于带 `auto` 的 Import Spec。

### 数据导出（Export Spec，工作台内执行）

导出与导入对称，但**不经 RPC 桥**：导出向导在数据工作台会话内直接执行（`runWorkbenchExportSpec` →
`buildExportCode(spec)` → `python_exec`），向导用保存对话框解析 `target.path` 后即落盘 / 写库 / 跑脚本。

Export Spec 字段：

```jsonc
{
  "format": "csv|tsv|excel|parquet|feather|json|jsonl|markdown|html|database|custom",
  "variableName": "要导出的会话变量",
  "target": {
    "path": "输出文件路径",
    "connStr": "数据库连接串", "table": "目标表名",
    "scriptText": "def dump(value, target): ...", "scriptPath": "/path/to/dumper.py", "entry": "dump"
  },
  "options": { "encoding": "utf-8-sig", "sep": ",", "index": false, "sheet": "Sheet1", "orient": "records", "indent": 2, "ifExists": "fail|replace|append" }
}
```

自定义导出脚本约定 `def dump(value, target)`，`target` 暴露 `.path` 与 `.write_bytes()/.write_text()`。

### Dataset Descriptor（导入结果记账）

导入成功后会写入 Cell 的 lab 导入记录，结构如下，便于后续“重新导入”与结构分析：

```jsonc
{
  "variableName": "sales_df",
  "dataKind": "table | structured | text | value",
  "sourceType": "file",
  "sourceLabel": "sales.xlsx",
  "source": { "category": "file", "format": "excel", "label": "sales.xlsx" },
  "schema": { "columns": ["month", "revenue"], "dtypes": {"revenue": "int64"}, "rowCount": 12, "shape": [12, 2] },
  "options": { "sheet": "2026" },
  "spec":   { /* 完整 Import Spec，可原样复跑 */ },
  "importedAt": "2026-06-03T10:00:00.000Z"
}
```

### import-source 示例

```js
await invoke('notebook_data_lab_rpc', {
  request: {
    action: 'import-source',
    filePath: 'E:/Record/source/Vue/guaxi/test/MarkdownTest.md',
    stableId: 'py-rs-44dd7ef0-1704',
    spec: {
      category: 'database',
      format: 'database',
      variableName: 'orders',
      source: { connStr: 'sqlite:///E:/data/shop.db', table: 'orders' },
    },
    clientId: 'frontend-demo',
  },
})
```

```bash
# 远程 API（JSON）→ 取 data.items 子数组并表格化
curl -X POST "http://127.0.0.1:8888/api/notebook/data-lab/action" \
  -H "Content-Type: application/json" \
  -d '{
    "action": "import-source",
    "filePath": "E:/Record/source/Vue/guaxi/test/MarkdownTest.md",
    "stableId": "py-rs-44dd7ef0-1704",
    "spec": {
      "category": "api", "format": "api-json", "variableName": "api_rows",
      "source": { "url": "https://example.com/api/list", "method": "GET", "headers": {"Accept": "application/json"}, "jsonPath": "data.items" }
    }
  }'
```

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
| `spec` | `object` | `import-source` 专用，完整 Import Spec（见下） |
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
