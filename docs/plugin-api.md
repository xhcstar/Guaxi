# Guaxi 插件 API

插件是放在 `utils/guaxi_editor_plugins/<id>/` 下的一个目录，里面至少包含 `manifest.json` 和入口脚本（默认 `main.py`）。入口脚本实现 `run(ctx)`，返回一个字典；宿主按插件声明的作用面（surface）解释返回值。

> 插件是**完整的 Python 进程内脚本**：不沙箱、可读写任意文件、可访问网络。只安装你信任的插件。

## 1. manifest.json

```json
{
  "id": "my-plugin",
  "name": "我的插件",
  "description": "一句话说明它做什么",
  "entry": "main.py",
  "capabilities": ["transform"],
  "surfaces": ["editor"],
  "scope": "selection",

  "args": [
    {
      "key": "mode",
      "label": "处理方式",
      "type": "select",
      "options": ["润色", "翻译"],
      "default": "润色",
      "placeholder": "",
      "required": false
    }
  ],

  "clipboard": false,
  "allow_empty_selection": false,
  "needs_kb": false
}
```

| 字段 | 说明 |
| --- | --- |
| `id` | 唯一标识；缺省时用目录名。启用状态与参数值都按它持久化。 |
| `name` / `description` | 设置页展示。 |
| `entry` | 入口脚本，默认 `main.py`。 |
| `capabilities` | 标签；编辑器只运行含 `transform` 的插件。 |
| `surfaces` | `editor` / `knowledge-base` / `repo`，默认 `editor`。 |
| `scope` | 语义标签，默认 `selection`；`insert` 表示“在光标处插入”的意图。 |
| `args` | 参数声明，见下节。 |
| `clipboard` | `true` 时运行前读取系统剪贴板，注入 `ctx.clipboard`。 |
| `allow_empty_selection` | `true` 时没有选区也能运行，结果插入光标处。 |
| `needs_kb` | `true` 时编辑器会先确保知识库列表已加载，再注入 `ctx.kb_location`。 |

### 参数（args）

设置页每个带参数的插件都有「参数」按钮，保存到 `_state.json` 的 `args.<plugin-id>` 下。运行插件时，宿主把「默认值 + 用户保存值」合并后放进 `ctx.args`；manifest 未声明的 key 会被忽略。

支持的 `type`：

| type | 设置页控件 | ctx.args 里的值 |
| --- | --- | --- |
| `text` | 单行输入 | `string` |
| `number` | 数字输入 | `number` |
| `bool` | 开关 | `boolean` |
| `select` | 下拉 | `string`（取自 `options`） |

参数是**持久默认值**，不是每次运行都弹窗。需要每次不同的值（文件路径、表单模板等）时，优先让插件从 `ctx.selection`、`ctx.clipboard` 或 `ctx.file_path` 里取。

## 2. 作用面与契约

### editor（编辑器）

触发：编辑器右键菜单 / 命令面板「运行插件」。

`ctx`：

| 字段 | 说明 |
| --- | --- |
| `selection` | 当前选中文本；无选区时为空串。 |
| `document` | 当前文档全文。 |
| `args` | 合并后的参数值。 |
| `file_path` | 当前编辑器绑定的文件绝对路径（可能为空）。 |
| `kb_location` | 当前选中的知识库目录（可能为空）。 |
| `kb_name` | 当前知识库名称。 |
| `clipboard` | 剪贴板文本；仅 `clipboard: true` 时读取，否则为空串。 |

返回：

```python
return {"text": "新文本"}          # 替换选区；空选区时插入光标处
return {"error": "给用户看的错误"}  # 不写回文档，直接提示
```

### knowledge-base（知识库）

从知识库标题栏的插件菜单触发。`ctx` 提供 `kb_name`、`kb_location`。

```python
return {"report": "# Markdown 报告"}  # 弹窗渲染
# 也可以返回 {"text": "..."}，前端会按 report 处理
```

### repo（文件库）

从文件库工具栏的插件菜单触发。

`ctx`：`knowledge_repo`、`resources`（资源元数据列表）、`count`。

```python
return {"report": "..."}  # 抽屉里渲染 Markdown
```

## 3. 可用能力

插件就是普通 Python，可以直接用标准库和内置运行时预装的包：

- 常用：`httpx`、`requests`-风格调用、`trafilatura`、`bs4`、`pandas`、`polars`、`openpyxl`、`duckdb`
- 文件：`open()`、`os.walk()`、`pathlib`
- 网络：任意出网请求
- 模型：`openai` 包（需自行配置），或走下面的本地接口

### 3.1 本地模型接口（推荐）

`POST http://127.0.0.1:8888/api/plugin/llm`

```json
{
  "system": "你是中文编辑",
  "prompt": "请润色：……",
  "model_id": null,
  "temperature": 0.3,
  "max_tokens": 4096
}
```

响应：

```json
{ "success": true, "text": "润色后的正文", "provider": "DeepSeek" }
{ "success": false, "text": "", "message": "没有可用的模型供应商……" }
```

- 供应商顺序：`model_id` 指定 > cc-switch > 自定义 > 内置（DeepSeek 优先）。
- API Key 由 Rust 侧从设置读取并解密，**不会进入 Python 进程**。
- 失败也返回 HTTP 200，请检查 `success` 字段。

### 3.2 MCP 工具

`POST http://127.0.0.1:8888/mcp`（JSON-RPC 2.0，loopback，无鉴权）

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "guaxi_task_query",
    "arguments": { "keyword": "周报" }
  }
}
```

常用工具：`guaxi_recall`、`guaxi_kb_semantic_search`、`guaxi_kb_stats`、`guaxi_kb_tags`、`guaxi_task_query`、`guaxi_search_text`、`web_search`、`web_fetch`、`brief_generate`。完整清单见 `src-tauri/src/cognition/tools.rs`。

### 3.3 采集入库

`POST http://0.0.0.0:14219/extension/capture/import`，见 `docs/capture-import-api.md`。

## 4. 内置插件

随应用首次运行写入，可自由修改/删除；`_builtins.json` 保证每个内置插件只补写一次。

| id | 作用面 | 说明 |
| --- | --- | --- |
| `llm-tool` | editor | 润色 / 精简 / 扩写 / 翻译 / 摘要 / 纠错 |
| `clip-table` | editor | 剪贴板 Excel/TSV 表格 → Markdown 表格 |
| `url-to-md` | editor | 选中网址 → 抓正文转 Markdown |
| `selection-to-kb` | editor | 选中内容 → 当前知识库笔记 + `[[双链]]` |
| `gen-data` | editor | 按列规格生成示例数据（Markdown/CSV/JSON） |
| `import-table` | editor | CSV/TSV/XLSX/JSON → Markdown 表格 |
| `form-fill` | editor | 笔记/任务字段 → 预填表单链接（默认只预览） |
| `md-table-total` | editor | Markdown 表格对齐 + 合计行 |
| `code-tidy` | editor | 代码块去空白、Tab 转空格、JSON 美化 |
| `cite-tidy` | editor | 参考文献重新编号 |
| `data-summary` | editor | 选区数字统计摘要 |
| `frontmatter-doctor` | knowledge-base | frontmatter 体检 |

## 5. 最小示例

```python
# main.py
def run(ctx):
    text = ctx.get("selection", "")
    return {"text": text.upper()}
```

带参数与剪贴板：

```json
{
  "id": "paste-upper",
  "name": "粘贴并大写",
  "entry": "main.py",
  "capabilities": ["transform"],
  "surfaces": ["editor"],
  "clipboard": true,
  "allow_empty_selection": true,
  "args": [
    { "key": "prefix", "label": "前缀", "type": "text", "default": "> " }
  ]
}
```

```python
def run(ctx):
    prefix = str((ctx.get("args") or {}).get("prefix") or "")
    body = str(ctx.get("clipboard") or "")
    if not body.strip():
        return {"error": "剪贴板为空"}
    return {"text": prefix + body}
```

## 6. 调试

1. 设置 → 插件 →「打开插件目录」，直接改 `main.py`。
2. 编辑器里选中文本 → 右键「运行插件」。
3. 失败时先看提示条里的 Python stderr；完整异常在 Python 运行日志里。
4. 想打印中间值：`print()` 会进入 worker 日志，不会写进文档。
5. 插件返回非 JSON 可序列化对象会导致整次调用失败，只返回字典/字符串/数字/布尔/列表。

## 7. 状态文件

`utils/guaxi_editor_plugins/_state.json`：

```json
{
  "disabled": ["some-plugin-id"],
  "args": {
    "llm-tool": { "mode": "翻译", "target_lang": "英文" }
  }
}
```

`_builtins.json` 记录已补写过的内置插件 id，不要手动删，否则内置插件会被重新生成。
