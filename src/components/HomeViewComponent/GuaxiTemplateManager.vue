<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <div class="header">
        <div class="header-main">
          <div class="title">模板管理器</div>
          <div class="header-summary">
            <span class="status-pill">{{ templateStorageStatus }}</span>
            <span>内置 {{ builtInTemplateCount }} 个</span>
            <span>自定义 {{ userTemplateCount }} 个</span>
            <span>分类 {{ templateCategoryCount }} 个</span>
          </div>
        </div>
        <div class="header-actions">
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button size="small" circle :loading="loading" aria-label="刷新模板列表" @click="refresh">
                <template #icon>
                  <Icon :icon="refreshCwIcon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            刷新模板列表
          </n-tooltip>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button size="small" circle :disabled="loading" aria-label="导入 Markdown 文件" @click="importFromFile">
                <template #icon>
                  <Icon :icon="uploadIcon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            导入 Markdown 文件
          </n-tooltip>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button size="small" circle type="primary" :disabled="loading" aria-label="新建模板" @click="newTemplate">
                <template #icon>
                  <Icon :icon="filePlusIcon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            新建模板
          </n-tooltip>
        </div>
      </div>

      <div class="main">
        <div class="left">
          <n-input v-model:value="filter" size="small" placeholder="搜索模板..." clearable />
          <div class="left-body">
            <n-collapse accordion>
              <n-collapse-item
                v-for="group in groupedTemplates"
                :key="group.type"
                :title="`${group.type} (${group.items.length})`"
                :name="group.type"
              >
                <n-list hoverable clickable>
                  <n-list-item
                    v-for="t in group.items"
                    :key="t.name"
                    :class="{ active: t.name === selectedName }"
                    @click="selectTemplate(t.name)"
                  >
                    <div class="row">
                      <div class="row-name">
                        <span class="row-name-text">{{ t.name }}</span>
                        <span v-if="t.builtin" class="builtin-tag">内置</span>
                      </div>
                      <n-tooltip trigger="hover" placement="left">
                        <template #trigger>
                          <n-button
                            size="tiny"
                            tertiary
                            circle
                            aria-label="插入模板到编辑器"
                            @click.stop="insertToEditor(t.name)"
                            :disabled="loading"
                          >
                            <template #icon>
                              <Icon :icon="sendIcon" width="14" height="14" />
                            </template>
                          </n-button>
                        </template>
                        插入到编辑器
                      </n-tooltip>
                    </div>
                  </n-list-item>
                </n-list>
              </n-collapse-item>
            </n-collapse>
          </div>
        </div>

        <div class="right">
          <n-form class="right-form" label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="名称">
              <n-input v-model:value="editName" :disabled="loading" placeholder="例如：会议纪要" />
            </n-form-item>
            <n-form-item label="类型">
              <n-input v-model:value="editType" :disabled="loading" placeholder="例如：会议 / 日记 / TODO" />
            </n-form-item>
            <n-form-item class="content-item" label="内容">
              <div ref="contentEditorContainer" class="template-monaco-editor"></div>
            </n-form-item>
          </n-form>

          <n-space justify="end" class="footer-actions">
            <n-tooltip trigger="hover" placement="top">
              <template #trigger>
                <n-button type="primary" :disabled="loading || !editName" @click="save">
                  <template #icon>
                    <Icon :icon="saveIcon" width="16" height="16" />
                  </template>
                  保存
                </n-button>
              </template>
              保存当前模板
            </n-tooltip>
            <n-tooltip trigger="hover" placement="top">
              <template #trigger>
                <n-button
                  tertiary
                  type="error"
                  :disabled="loading || !selectedName"
                  @click="remove"
                >
                  <template #icon>
                    <Icon :icon="trash2Icon" width="16" height="16" />
                  </template>
                  删除
                </n-button>
              </template>
              删除当前自定义模板
            </n-tooltip>
          </n-space>
        </div>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { NButton, NCollapse, NCollapseItem, NForm, NFormItem, NInput, NList, NListItem, NSpace, NTooltip, createDiscreteApi } from 'naive-ui'
import { Icon } from '@iconify/vue'
import * as monaco from 'monaco-editor'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import filePlusIcon from '@iconify-icons/lucide/file-plus'
import refreshCwIcon from '@iconify-icons/lucide/refresh-cw'
import saveIcon from '@iconify-icons/lucide/save'
import sendIcon from '@iconify-icons/lucide/send'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import uploadIcon from '@iconify-icons/lucide/upload'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const logger = createLogger('GuaxiTemplateManager')
const { message } = createDiscreteApi(['message'])

const loading = ref(false)
const templateRepoPath = ref('')
const templates = ref([])
const filter = ref('')

const selectedName = ref('')
const editName = ref('')
const editType = ref('未分类')
const editContent = ref('')
const contentEditorContainer = ref(null)

const TEMPLATE_EDITOR_FONT_FAMILY = "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'"
const TEMPLATE_EDITOR_FONT_SIZE = 18
let contentEditor = null
let contentModel = null
let suppressEditorChange = false
let themeObserver = null

const PYTHON_CELL_TEMPLATE_GROUP = '探索者 / Python Cell'
const py = String.raw

// 设计说明：
// 1. Python Cell 模板统一输出 Guaxi 已支持的 <!--py--> 注释哨兵语法，插入后可被普通编辑器和探索者模式共同识别。
// 2. 模板按数据读取、清洗、整理、处理、可视化、导出、网络、文本、多媒体、二进制等常见工作流拆分，避免用户在一个超大模板里反复删改。
// 3. title 元数据会显示在 Python Cell 卡片上，帮助探索者模式里快速辨认每个代码块的用途。
const buildPythonCellTemplate = (name, category, code) => ({
  name: `Python Cell｜${name}`,
  templateType: `${PYTHON_CELL_TEMPLATE_GROUP} / ${category}`,
  content: `<!--py {"title":"${name}"}-->\n${String(code || '').trim()}\n<!--/py-->\n`
})

const PYTHON_CELL_TEMPLATES = [
  buildPythonCellTemplate('数据读取：CSV / Excel / JSON', '数据读取', py`
from pathlib import Path
import json
import pandas as pd

DATA_DIR = Path("data")
csv_path = DATA_DIR / "sample.csv"
excel_path = DATA_DIR / "sample.xlsx"
json_path = DATA_DIR / "sample.json"

# 根据真实文件类型保留一种读取方式即可。
df_csv = pd.read_csv(csv_path, encoding="utf-8")
df_excel = pd.read_excel(excel_path, sheet_name=0)
with json_path.open("r", encoding="utf-8") as f:
    raw_json = json.load(f)
df_json = pd.json_normalize(raw_json)

print(df_csv.head())
print(df_csv.info())
`),
  buildPythonCellTemplate('数据读取：SQLite 查询', '数据读取', py`
from pathlib import Path
import sqlite3
import pandas as pd

db_path = Path("data/app.db")
sql = """
SELECT *
FROM your_table
LIMIT 100
"""

with sqlite3.connect(db_path) as conn:
    df = pd.read_sql_query(sql, conn)

print(df.head())
print(f"rows={len(df)}, columns={len(df.columns)}")
`),
  buildPythonCellTemplate('数据清洗：缺失值 / 重复值 / 类型', '数据清洗', py`
import pandas as pd

df = pd.read_csv("data/raw.csv", encoding="utf-8")

print("原始形状:", df.shape)
print("缺失值统计:")
print(df.isna().sum().sort_values(ascending=False).head(20))

df = df.drop_duplicates()
df.columns = [str(c).strip().lower().replace(" ", "_") for c in df.columns]

for col in df.select_dtypes(include="object").columns:
    df[col] = df[col].astype(str).str.strip()

if "date" in df.columns:
    df["date"] = pd.to_datetime(df["date"], errors="coerce")

df = df.dropna(how="all")
print("清洗后形状:", df.shape)
df.head()
`),
  buildPythonCellTemplate('数据整理：分组聚合 / 透视表', '数据整理', py`
import pandas as pd

df = pd.read_csv("data/clean.csv", encoding="utf-8")

summary = (
    df.groupby(["category"], dropna=False)
      .agg(
          count=("category", "size"),
          amount_sum=("amount", "sum"),
          amount_mean=("amount", "mean"),
      )
      .reset_index()
      .sort_values("amount_sum", ascending=False)
)

pivot = pd.pivot_table(
    df,
    index="category",
    columns="month",
    values="amount",
    aggfunc="sum",
    fill_value=0,
)

print(summary.head(20))
pivot.head()
`),
  buildPythonCellTemplate('数据整理：多表合并', '数据整理', py`
import pandas as pd

orders = pd.read_csv("data/orders.csv", encoding="utf-8")
users = pd.read_csv("data/users.csv", encoding="utf-8")

merged = orders.merge(
    users,
    how="left",
    on="user_id",
    validate="many_to_one",
)

missing_user = merged["user_id"].isna().sum()
print(f"合并后行数: {len(merged)}, 缺失 user_id: {missing_user}")
merged.head()
`),
  buildPythonCellTemplate('数据处理：特征工程 / 时间窗口', '数据处理', py`
import pandas as pd

df = pd.read_csv("data/events.csv", encoding="utf-8")
df["event_time"] = pd.to_datetime(df["event_time"], errors="coerce")
df = df.sort_values(["user_id", "event_time"])

df["amount"] = pd.to_numeric(df["amount"], errors="coerce").fillna(0)
df["amount_7d_avg"] = (
    df.groupby("user_id")["amount"]
      .rolling(window=7, min_periods=1)
      .mean()
      .reset_index(level=0, drop=True)
)
df["event_hour"] = df["event_time"].dt.hour
df["is_weekend"] = df["event_time"].dt.dayofweek >= 5

df.head()
`),
  buildPythonCellTemplate('绘图：趋势线 / 分布图', '绘图', py`
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv("data/clean.csv", encoding="utf-8")
df["date"] = pd.to_datetime(df["date"], errors="coerce")

daily = (
    df.dropna(subset=["date"])
      .groupby(pd.Grouper(key="date", freq="D"))["amount"]
      .sum()
)

fig, axes = plt.subplots(1, 2, figsize=(12, 4))
daily.plot(ax=axes[0], title="Daily Amount")
df["amount"].dropna().hist(ax=axes[1], bins=30)
axes[1].set_title("Amount Distribution")
plt.tight_layout()
plt.show()
`),
  buildPythonCellTemplate('绘图：相关性热力图', '绘图', py`
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv("data/clean.csv", encoding="utf-8")
numeric = df.select_dtypes(include="number")
corr = numeric.corr()

fig, ax = plt.subplots(figsize=(8, 6))
im = ax.imshow(corr, cmap="RdBu_r", vmin=-1, vmax=1)
ax.set_xticks(range(len(corr.columns)), corr.columns, rotation=45, ha="right")
ax.set_yticks(range(len(corr.index)), corr.index)
fig.colorbar(im, ax=ax, fraction=0.046, pad=0.04)
ax.set_title("Correlation Heatmap")
plt.tight_layout()
plt.show()
`),
  buildPythonCellTemplate('数据导出：CSV / Excel / Markdown', '数据导出', py`
from pathlib import Path
import pandas as pd

output_dir = Path("output")
output_dir.mkdir(parents=True, exist_ok=True)

df = pd.read_csv("data/clean.csv", encoding="utf-8")

df.to_csv(output_dir / "result.csv", index=False, encoding="utf-8-sig")
df.to_excel(output_dir / "result.xlsx", index=False)

markdown_table = df.head(20).to_markdown(index=False)
(output_dir / "preview.md").write_text(markdown_table, encoding="utf-8")

print(f"已导出到: {output_dir.resolve()}")
`),
  buildPythonCellTemplate('网络抓取：网页下载 / 链接提取', '网络抓取', py`
from html.parser import HTMLParser
from urllib.parse import urljoin
from urllib.request import Request, urlopen

class LinkParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        if tag != "a":
            return
        attrs = dict(attrs)
        href = attrs.get("href")
        text = attrs.get("title", "")
        if href:
            self.links.append((href, text))

url = "https://example.com"
request = Request(url, headers={"User-Agent": "GuaxiTemplate/1.0"})
html = urlopen(request, timeout=20).read().decode("utf-8", errors="ignore")

parser = LinkParser()
parser.feed(html)
for href, text in parser.links[:20]:
    print(urljoin(url, href), text)
`),
  buildPythonCellTemplate('网络抓取：分页 API JSON', '网络抓取', py`
import time
from urllib.parse import urlencode
from urllib.request import Request, urlopen
import json
import pandas as pd

base_url = "https://api.example.com/items"
all_items = []

for page in range(1, 6):
    query = urlencode({"page": page, "page_size": 100})
    request = Request(f"{base_url}?{query}", headers={"User-Agent": "GuaxiTemplate/1.0"})
    with urlopen(request, timeout=20) as response:
        payload = json.loads(response.read().decode("utf-8"))
    items = payload.get("items", payload if isinstance(payload, list) else [])
    all_items.extend(items)
    print(f"page={page}, items={len(items)}")
    time.sleep(0.5)

df = pd.json_normalize(all_items)
df.head()
`),
  buildPythonCellTemplate('文本处理：正则清洗 / 词频统计', '文本处理', py`
from collections import Counter
from pathlib import Path
import re

text_path = Path("data/input.txt")
text = text_path.read_text(encoding="utf-8")

clean_text = re.sub(r"\s+", " ", text)
tokens = re.findall(r"[\w\u4e00-\u9fff]+", clean_text.lower())
stopwords = {"the", "a", "and", "的", "了", "是"}
tokens = [token for token in tokens if token not in stopwords and len(token) > 1]

counter = Counter(tokens)
for word, count in counter.most_common(30):
    print(word, count)
`),
  buildPythonCellTemplate('图片处理：批量缩放 / 转格式', '图片视频处理', py`
from pathlib import Path

from PIL import Image

input_dir = Path("images")
output_dir = Path("output/images")
output_dir.mkdir(parents=True, exist_ok=True)

for path in input_dir.glob("*"):
    if path.suffix.lower() not in {".png", ".jpg", ".jpeg", ".webp"}:
        continue
    with Image.open(path) as img:
        img.thumbnail((1280, 1280))
        target = output_dir / f"{path.stem}.jpg"
        img.convert("RGB").save(target, quality=88, optimize=True)
        print(f"{path.name} -> {target.name}")
`),
  buildPythonCellTemplate('视频处理：抽帧', '图片视频处理', py`
from pathlib import Path

import cv2

video_path = Path("videos/input.mp4")
output_dir = Path("output/frames")
output_dir.mkdir(parents=True, exist_ok=True)

cap = cv2.VideoCapture(str(video_path))
fps = cap.get(cv2.CAP_PROP_FPS) or 25
interval = int(fps * 2)
frame_index = 0
saved = 0

while True:
    ok, frame = cap.read()
    if not ok:
        break
    if frame_index % interval == 0:
        target = output_dir / f"frame_{frame_index:06d}.jpg"
        cv2.imwrite(str(target), frame)
        saved += 1
    frame_index += 1

cap.release()
print(f"保存帧数: {saved}")
`),
  buildPythonCellTemplate('二进制文件：哈希 / Hex 预览', '二进制文件处理', py`
from pathlib import Path
import hashlib

path = Path("data/sample.bin")
data = path.read_bytes()

sha256 = hashlib.sha256(data).hexdigest()
head = data[:128]

print(f"file={path}")
print(f"size={len(data)} bytes")
print(f"sha256={sha256}")
print("hex preview:")
print(head.hex(" ", 1))
`),
  buildPythonCellTemplate('二进制文件：struct 解析记录', '二进制文件处理', py`
from pathlib import Path
import struct

path = Path("data/records.bin")
record_struct = struct.Struct("<I f 16s")

data = path.read_bytes()
records = []
for offset in range(0, len(data), record_struct.size):
    chunk = data[offset:offset + record_struct.size]
    if len(chunk) < record_struct.size:
        break
    item_id, score, raw_name = record_struct.unpack(chunk)
    name = raw_name.rstrip(b"\x00").decode("utf-8", errors="ignore")
    records.append({"id": item_id, "score": score, "name": name})

print(records[:10])
print(f"records={len(records)}")
`),
  buildPythonCellTemplate('文件批处理：目录扫描 / 清单导出', '文件处理', py`
from pathlib import Path
import pandas as pd

root = Path(".")
rows = []

for path in root.rglob("*"):
    if path.is_file():
        stat = path.stat()
        rows.append({
            "path": str(path),
            "suffix": path.suffix.lower(),
            "size": stat.st_size,
            "modified": stat.st_mtime,
        })

df = pd.DataFrame(rows).sort_values("size", ascending=False)
df.to_csv("output/file_inventory.csv", index=False, encoding="utf-8-sig")
df.head(30)
`)
]

const DEFAULT_TEMPLATES = [
  {
    name: '会议纪要（通用）',
    templateType: '会议',
    content: '# 📝 会议纪要\n\n## 基本信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 会议主题 |  |\n| 时间 |  |\n| 地点/会议链接 |  |\n| 主持人 |  |\n| 记录人 |  |\n| 参会人员 |  |\n| 缺席人员 |  |\n\n---\n\n## 会议目标\n\n- 目标 1：\n- 目标 2：\n\n## 议程\n\n1. \n2. \n3. \n\n---\n\n## 讨论纪要（按议题）\n\n### 议题 1：\n\n- 背景：\n- 讨论要点：\n  - \n- 结论/共识：\n- 未决问题：\n  - \n\n### 议题 2：\n\n- 背景：\n- 讨论要点：\n  - \n- 结论/共识：\n- 未决问题：\n  - \n\n---\n\n## 决议（Decisions）\n\n| 序号 | 决议 | 影响范围 | 生效时间 | 备注 |\n| --- | --- | --- | --- | --- |\n| 1 |  |  |  |  |\n\n## 行动项（Action Items）\n\n| 序号 | 事项 | 负责人 | 截止日期 | 状态 | 关联议题 |\n| --- | --- | --- | --- | --- | --- |\n| 1 |  |  |  | 待开始 |  |\n\n## 风险与依赖\n\n- 风险：\n- 依赖：\n\n## 附录\n\n- 资料链接：\n- 截图/录屏：\n'
  },
  {
    name: '日报',
    templateType: '工作',
    content: '# 🗓️ 日报\n\n## 基本信息\n\n- 日期：\n- 负责人：\n- 所属项目/团队：\n\n---\n\n## 今日完成（Done）\n\n- [ ] 事项 1（结果/产出：）\n- [ ] 事项 2（结果/产出：）\n\n## 今日产出物\n\n| 类型 | 链接/路径 | 说明 |\n| --- | --- | --- |\n| PR/提交 |  |  |\n| 文档 |  |  |\n| 会议纪要 |  |  |\n\n## 进展与数据（可选）\n\n- 进度：%\n- 关键指标：\n\n---\n\n## 问题/风险（Blockers & Risks）\n\n| 问题/风险 | 影响 | 当前状态 | 需要谁协助 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n\n## 明日计划（Next）\n\n- [ ] 事项 1\n- [ ] 事项 2\n\n## 备注\n\n- \n'
  },
  {
    name: '周报',
    templateType: '工作',
    content: '# 📆 周报\n\n## 基本信息\n\n- 周期：\n- 负责人：\n- 项目/团队：\n\n---\n\n## 本周概览\n\n- 本周重点：\n- 总体进度：\n\n## 本周产出\n\n| 类别 | 条目 | 说明 |\n| --- | --- | --- |\n| 功能 |  |  |\n| 修复 |  |  |\n| 性能/体验 |  |  |\n| 文档/流程 |  |  |\n\n## 关键数据（可选）\n\n| 指标 | 本周 | 上周 | 环比 | 备注 |\n| --- | --- | --- | --- | --- |\n|  |  |  |  |  |\n\n---\n\n## 问题与风险\n\n- 问题：\n- 风险：\n- 复盘点：\n\n## 下周计划\n\n- [ ] 事项 1（交付物：）\n- [ ] 事项 2（交付物：）\n\n## 需要协助\n\n| 事项 | 需要谁 | 希望时间 | 说明 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n'
  },
  {
    name: 'TODO（清单）',
    templateType: 'TODO',
    content: '# ✅ TODO\n\n## 今日（Today）\n\n- [ ] 最重要的一件事：\n- [ ] \n- [ ] \n\n## 本周（This Week）\n\n- [ ] \n- [ ] \n\n## 待办池（Backlog）\n\n- [ ] \n- [ ] \n\n---\n\n## 进行中（WIP）\n\n| 事项 | 优先级 | 预计完成 | 状态 | 备注 |\n| --- | --- | --- | --- | --- |\n|  | P1 |  | 进行中 |  |\n\n## 已完成（Done）\n\n- [x] \n'
  },
  {
    name: '阅读笔记',
    templateType: '学习',
    content: '# 📚 阅读笔记\n\n## 书目信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 书/文章 |  |\n| 作者 |  |\n| 来源/链接 |  |\n| 阅读时间 |  |\n| 主题标签 |  |\n\n---\n\n## 一句话总结\n\n> \n\n## 摘要（Summary）\n\n- \n\n## 核心观点（Key Points）\n\n1. \n2. \n3. \n\n## 证据与例子（Evidence）\n\n- 例子 1：\n- 例子 2：\n\n## 金句/引用（Quotes）\n\n> \n\n## 我的思考（Thoughts）\n\n- 同意的点：\n- 不同意的点：\n- 可以迁移到哪里：\n\n## 可行动项（Actions）\n\n- [ ] \n- [ ] \n\n## 相关链接\n\n- \n'
  },
  {
    name: '学习笔记（课程/视频）',
    templateType: '学习',
    content: '# 🎓 学习笔记\n\n## 基本信息\n\n- 主题：\n- 来源：\n- 学习时间：\n- 讲师/作者：\n- 关联项目：\n\n---\n\n## 学习目标\n\n- 目标 1：\n- 目标 2：\n\n## 知识点结构\n\n- 概念：\n- 方法：\n- 工具：\n- 注意事项：\n\n## 关键结论\n\n1. \n2. \n\n## 例子 / 代码\n\n```\n\n```\n\n## 常见坑\n\n- \n\n## 疑问与待查\n\n- [ ] \n\n## 复盘\n\n- 今天最大的收获：\n- 下一步练习：\n'
  },
  {
    name: '项目需求（PRD 简版）',
    templateType: '项目',
    content: '# 📌 需求说明（PRD 简版）\n\n## 基本信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 需求名称 |  |\n| 负责人 |  |\n| 关联项目 |  |\n| 版本/里程碑 |  |\n| 优先级 | P0 / P1 / P2 |\n| 评审时间 |  |\n\n---\n\n## 背景与问题\n\n- 现状：\n- 痛点：\n- 为什么现在做：\n\n## 目标（Goals）\n\n- 业务目标：\n- 用户目标：\n- 成功指标（可量化）：\n\n## 非目标（Non-goals）\n\n- 本期不做：\n\n---\n\n## 用户画像与场景\n\n- 用户 A：\n- 场景：\n\n## 方案概述\n\n- 方案一：\n- 方案二：\n- 选型结论：\n\n## 需求范围\n\n### In Scope\n\n- \n\n### Out of Scope\n\n- \n\n---\n\n## 功能列表\n\n| 模块 | 功能点 | 描述 | 优先级 | 备注 |\n| --- | --- | --- | --- | --- |\n|  |  |  | P1 |  |\n\n## 交互/原型\n\n- 原型链接：\n- 关键页面说明：\n\n## 数据与埋点（可选）\n\n| 事件 | 触发时机 | 属性 | 用途 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n\n## 风险与依赖\n\n- 风险：\n- 依赖：\n\n## 验收标准\n\n- [ ] 功能验收：\n- [ ] 性能/稳定性：\n- [ ] 兼容性：\n- [ ] 回滚预案：\n'
  },
  {
    name: 'Bug 报告',
    templateType: '项目',
    content: '# 🐞 Bug 报告\n\n## 基本信息\n\n- 标题：\n- 严重程度：致命 / 高 / 中 / 低\n- 优先级：P0 / P1 / P2\n- 影响范围：\n- 发现版本/分支：\n- 环境：系统/浏览器/设备/网络\n\n---\n\n## 复现步骤\n\n1. \n2. \n3. \n\n## 实际结果（Actual）\n\n- \n\n## 期望结果（Expected）\n\n- \n\n---\n\n## 复现概率\n\n- 必现 / 高概率 / 低概率 / 偶现\n\n## 影响评估\n\n- 用户影响：\n- 业务影响：\n- 是否可绕过：是 / 否（绕过方式：）\n\n## 日志/截图/录屏\n\n- \n\n## 初步定位（可选）\n\n- 可疑模块：\n- 最近变更：\n- 相关提交/PR：\n\n## 修复建议（可选）\n\n- \n\n## 验证要点\n\n- [ ] 主路径\n- [ ] 边界条件\n- [ ] 回归范围\n'
  },
  {
    name: '日记',
    templateType: '日记',
    content: '# 🌙 日记\n\n## 今日信息\n\n- 日期：\n- 心情：\n- 天气：\n- 关键词：\n\n---\n\n## 记录\n\n### 今天发生了什么\n\n- \n\n### 今天我做得好的地方\n\n- \n\n### 今天的困扰与应对\n\n- 困扰：\n- 我采取的行动：\n- 结果：\n\n---\n\n## 反思与成长\n\n- 我学到了什么：\n- 下一次我想怎么做：\n\n## 感谢\n\n- \n\n## 明日计划\n\n- [ ] \n'
  },
  ...PYTHON_CELL_TEMPLATES
]

const TEMPLATE_TYPE_ORDER = [
  '会议',
  '工作',
  'TODO',
  '学习',
  '项目',
  '日记',
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据读取`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据清洗`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据整理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 绘图`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据导出`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 网络抓取`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 文本处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 图片视频处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 二进制文件处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 文件处理`,
  '未分类'
]

const builtInTemplateCount = DEFAULT_TEMPLATES.length
const userTemplateCount = computed(() => (templates.value || []).length)
const templateStorageStatus = computed(() => templateRepoPath.value ? '模板库已连接' : '仅显示内置模板')

const defineTemplateEditorThemes = () => {
  // 设计说明：模板管理器是独立窗口时不一定会先加载主编辑器，因此这里注册一份轻量主题兜底。
  // 字体与字号继续使用主编辑器同款配置，颜色仅做面板级适配，避免 Monaco 回落到刺眼默认背景。
  monaco.editor.defineTheme('guaxi-template-light', {
    base: 'vs',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2937',
      'editorLineNumber.foreground': '#8a94a6',
      'editorCursor.foreground': '#0f766e',
      'editor.selectionBackground': '#99f6e455',
      'editor.lineHighlightBackground': '#f6f8fa',
      'editor.lineHighlightBorder': '#00000000',
    },
  })
  monaco.editor.defineTheme('guaxi-template-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#1f2329',
      'editor.foreground': '#d7dde8',
      'editorLineNumber.foreground': '#6b7280',
      'editorCursor.foreground': '#61afef',
      'editor.selectionBackground': '#264f78',
      'editor.lineHighlightBackground': '#2a2f38',
      'editor.lineHighlightBorder': '#00000000',
    },
  })
}

const getTemplateEditorTheme = () => {
  const theme = document.documentElement?.getAttribute('data-guaxi-theme') || 'light'
  return theme === 'dark' ? 'guaxi-template-dark' : 'guaxi-template-light'
}

const applyTemplateEditorTheme = () => {
  if (!contentEditor) return
  monaco.editor.setTheme(getTemplateEditorTheme())
}

const setEditorContent = (value) => {
  if (!contentModel) return
  const next = String(value || '')
  if (contentModel.getValue() === next) return
  suppressEditorChange = true
  contentModel.setValue(next)
  suppressEditorChange = false
}

const initContentEditor = () => {
  if (contentEditor || !contentEditorContainer.value) return

  defineTemplateEditorThemes()
  const uri = monaco.Uri.parse(`inmemory://guaxi-template-manager/template-${Date.now()}.md`)
  contentModel = monaco.editor.createModel(String(editContent.value || ''), 'markdown', uri)
  contentEditor = monaco.editor.create(contentEditorContainer.value, {
    model: contentModel,
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    lineNumbers: 'on',
    renderWhitespace: 'selection',
    tabSize: 2,
    insertSpaces: true,
    fontFamily: TEMPLATE_EDITOR_FONT_FAMILY,
    fontSize: TEMPLATE_EDITOR_FONT_SIZE,
    lineHeight: 28,
    readOnly: loading.value,
    theme: getTemplateEditorTheme(),
  })

  contentModel.onDidChangeContent(() => {
    if (suppressEditorChange) return
    editContent.value = contentModel?.getValue() || ''
  })

  themeObserver = new MutationObserver(() => applyTemplateEditorTheme())
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-guaxi-theme'],
  })

  logger.info('[TemplateManager] Monaco editor initialized', {
    fontFamily: TEMPLATE_EDITOR_FONT_FAMILY,
    fontSize: TEMPLATE_EDITOR_FONT_SIZE,
  })
}

const getTemplateTypeRank = (type) => {
  const index = TEMPLATE_TYPE_ORDER.indexOf(String(type || ''))
  return index >= 0 ? index : TEMPLATE_TYPE_ORDER.length
}

const hasUserTemplate = (name) => {
  const n = String(name || '').trim()
  if (!n) return false
  return (templates.value || []).some(t => String(t?.name || '').trim() === n)
}

const getBuiltInTemplate = (name) => {
  const n = String(name || '').trim()
  if (!n) return null
  return DEFAULT_TEMPLATES.find(t => String(t?.name || '').trim() === n) || null
}

const refresh = async () => {
  try {
    loading.value = true
    logger.info('[TemplateManager] refresh requested')
    const [path, list] = await Promise.all([
      invoke('get_template_repo_path'),
      invoke('list_templates')
    ])
    templateRepoPath.value = typeof path === 'string' ? path : ''
    templates.value = Array.isArray(list) ? list : []
    logger.info('[TemplateManager] refresh finished', {
      pathConfigured: !!templateRepoPath.value,
      builtinCount: builtInTemplateCount,
      userCount: templates.value.length,
    })
  } catch (e) {
    logger.error('加载模板失败:', e)
    message.error('加载模板失败')
  } finally {
    loading.value = false
  }
}

const mergedTemplates = computed(() => {
  const byName = new Map()

  for (const t of DEFAULT_TEMPLATES) {
    const name = String(t?.name || '').trim()
    if (!name) continue
    const tp = String(t?.templateType || '未分类').trim() || '未分类'
    byName.set(name, {
      name,
      template_type: tp,
      builtin: true,
    })
  }

  for (const t of templates.value || []) {
    const name = String(t?.name || '').trim()
    if (!name) continue
    const tp = String(t?.template_type || '未分类').trim() || '未分类'
    byName.set(name, {
      name,
      template_type: tp,
      builtin: false,
    })
  }

  const q = filter.value.trim().toLowerCase()
  let list = Array.from(byName.values())
  if (q) {
    list = list.filter(t => {
      const n = String(t?.name || '').toLowerCase()
      const tp = String(t?.template_type || '').toLowerCase()
      return n.includes(q) || tp.includes(q)
    })
  }

  return list
})

const groupedTemplates = computed(() => {
  const map = new Map()
  for (const t of mergedTemplates.value) {
    const tp = String(t?.template_type || '未分类') || '未分类'
    if (!map.has(tp)) map.set(tp, [])
    map.get(tp).push(t)
  }
  return Array.from(map.entries()).map(([type, items]) => ({
    type,
    items: items.slice().sort((a, b) => String(a.name).localeCompare(String(b.name)))
  })).sort((a, b) => {
    const rankDiff = getTemplateTypeRank(a.type) - getTemplateTypeRank(b.type)
    if (rankDiff !== 0) return rankDiff
    return String(a.type).localeCompare(String(b.type))
  })
})

const templateCategoryCount = computed(() => groupedTemplates.value.length)

const newTemplate = () => {
  logger.info('[TemplateManager] create draft template')
  selectedName.value = ''
  editName.value = ''
  editType.value = '未分类'
  editContent.value = ''
}

const selectTemplate = async (name) => {
  const n = String(name || '').trim()
  if (!n) return

  try {
    loading.value = true
    logger.info('[TemplateManager] select template', { name: n, builtin: !!getBuiltInTemplate(n), user: hasUserTemplate(n) })

    if (hasUserTemplate(n)) {
      const data = await invoke('read_template', { name: n })
      selectedName.value = data?.name || n
      editName.value = data?.name || n
      editType.value = data?.template_type || '未分类'
      editContent.value = data?.content || ''
      return
    }

    const builtIn = getBuiltInTemplate(n)
    if (builtIn) {
      selectedName.value = builtIn.name
      editName.value = builtIn.name
      editType.value = builtIn.templateType || '未分类'
      editContent.value = builtIn.content || ''
      logger.info('[TemplateManager] selected built-in template', {
        name: builtIn.name,
        templateType: builtIn.templateType || '未分类',
      })
      return
    }

    const data = await invoke('read_template', { name: n })
    selectedName.value = data?.name || n
    editName.value = data?.name || n
    editType.value = data?.template_type || '未分类'
    editContent.value = data?.content || ''
  } catch (e) {
    logger.error('读取模板失败:', e)
    message.error('读取模板失败')
  } finally {
    loading.value = false
  }
}

const save = async () => {
  const name = editName.value.trim()
  if (!name) {
    logger.info('[TemplateManager] save skipped: empty template name')
    message.warning('请输入模板名称')
    return
  }

  try {
    loading.value = true
    logger.info('[TemplateManager] save template requested', {
      previousName: selectedName.value,
      nextName: name,
      templateType: editType.value.trim() || '未分类',
    })

    if (selectedName.value && selectedName.value !== name && hasUserTemplate(selectedName.value)) {
      await invoke('rename_template', { oldName: selectedName.value, newName: name })
      selectedName.value = name
    } else {
      selectedName.value = name
    }

    await invoke('save_template', {
      name,
      templateType: editType.value.trim() || '未分类',
      content: editContent.value || ''
    })

    message.success('已保存')
    logger.info('[TemplateManager] save template finished', { name })
    await refresh()
  } catch (e) {
    logger.error('保存模板失败:', e)
    message.error('保存模板失败')
  } finally {
    loading.value = false
  }
}

const remove = async () => {
  if (!selectedName.value) return

  if (!hasUserTemplate(selectedName.value) && getBuiltInTemplate(selectedName.value)) {
    logger.info('[TemplateManager] delete skipped for built-in template', { name: selectedName.value })
    message.info('内置模板不可删除')
    return
  }

  const ok = window.confirm(`确认删除模板「${selectedName.value}」？`)
  if (!ok) {
    logger.info('[TemplateManager] delete cancelled', { name: selectedName.value })
    return
  }

  try {
    loading.value = true
    logger.info('[TemplateManager] delete template requested', { name: selectedName.value })
    await invoke('delete_template', { name: selectedName.value })
    message.success('已删除')
    newTemplate()
    await refresh()
  } catch (e) {
    logger.error('删除模板失败:', e)
    message.error('删除模板失败')
  } finally {
    loading.value = false
  }
}

const insertToEditor = async (name) => {
  const n = String(name || '').trim()
  if (!n) return

  try {
    loading.value = true
    logger.info('[TemplateManager] insert template requested', { name: n, builtin: !!getBuiltInTemplate(n), user: hasUserTemplate(n) })

    if (hasUserTemplate(n)) {
      const data = await invoke('read_template', { name: n })
      const content = typeof data?.content === 'string' ? data.content : ''
      if (!content) {
        logger.info('[TemplateManager] insert skipped: empty user template', { name: n })
        return
      }
      await invoke('emit_template_insert', { content })
      message.success('已发送到编辑器窗口')
      logger.info('[TemplateManager] inserted user template', { name: n, length: content.length })
      return
    }

    const builtIn = getBuiltInTemplate(n)
    const content = typeof builtIn?.content === 'string' ? builtIn.content : ''
    if (!content) {
      logger.info('[TemplateManager] insert skipped: empty built-in template', { name: n })
      return
    }
    await invoke('emit_template_insert', { content })
    message.success('已发送到编辑器窗口')
    logger.info('[TemplateManager] inserted built-in template', { name: n, length: content.length })
  } catch (e) {
    logger.error('插入失败:', e)
    message.error('插入失败')
  } finally {
    loading.value = false
  }
}

const importFromFile = async () => {
  try {
    logger.info('[TemplateManager] import markdown requested')
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Markdown', extensions: ['md', 'markdown'] }]
    })
    if (!selected) {
      logger.info('[TemplateManager] import markdown cancelled')
      return
    }

    const defaultName = String(selected).split(/[\\/]/).pop()?.replace(/\.(md|markdown)$/i, '') || '新模板'
    const name = window.prompt('模板名称', defaultName) || ''
    if (!name.trim()) {
      logger.info('[TemplateManager] import skipped: empty template name', { sourcePath: selected })
      return
    }
    const type = window.prompt('模板类型（用于分类）', '未分类') || '未分类'

    loading.value = true
    logger.info('[TemplateManager] import markdown confirmed', {
      sourcePath: selected,
      name: name.trim(),
      templateType: type.trim() || '未分类',
    })
    await invoke('import_template_from_file', {
      sourcePath: selected,
      name: name.trim(),
      templateType: type.trim() || '未分类'
    })
    message.success('导入成功')
    logger.info('[TemplateManager] import markdown finished', { name: name.trim() })
    await refresh()
  } catch (e) {
    logger.error('导入失败:', e)
    message.error('导入失败')
  } finally {
    loading.value = false
  }
}

watch(editContent, (value) => {
  setEditorContent(value)
})

watch(loading, (value) => {
  contentEditor?.updateOptions?.({ readOnly: !!value })
})

onMounted(async () => {
  await nextTick()
  initContentEditor()
  await refresh()
})

onBeforeUnmount(() => {
  themeObserver?.disconnect?.()
  themeObserver = null
  contentEditor?.dispose?.()
  contentEditor = null
  contentModel?.dispose?.()
  contentModel = null
})
</script>

<style scoped>
.page {
  height: 100vh;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
  gap: 10px;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.header-main {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 1 1 auto;
}

.title {
  flex: 0 0 auto;
  font-size: 16px;
  font-weight: 650;
  letter-spacing: 0.2px;
}

.header-summary {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  white-space: nowrap;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  line-height: 22px;
}

.header-summary > span {
  flex: 0 0 auto;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.status-pill {
  color: var(--guaxi-text-primary);
  font-weight: 650;
  padding: 0 8px;
  border-radius: 999px;
  background: rgba(45, 212, 191, 0.12);
}

.main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 360px 1fr;
  grid-template-rows: minmax(0, 1fr);
  gap: 12px;
  overflow: hidden;
}

.left {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.left-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.right {
  height: 100%;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 12px;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 28px rgba(16, 24, 40, 0.08);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.right-form {
  flex: 1 1 0%;
  min-height: 0;
  height: 100%;
  display: flex !important;
  flex-direction: column;
  overflow: hidden;
}

.content-item {
  flex: 1 1 0% !important;
  min-height: 0;
  display: flex;
  align-items: stretch;
}

.content-item :deep(.n-form-item-label) {
  flex: 0 0 auto;
  align-self: flex-start;
}

.content-item :deep(.n-form-item-blank) {
  flex: 1 1 0%;
  min-height: 0;
  height: 100%;
  align-self: stretch;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.template-monaco-editor {
  flex: 1;
  min-height: 0;
  height: 100%;
  width: 100%;
  box-sizing: border-box;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-editor-bg, var(--guaxi-panel-bg));
}

.template-monaco-editor :deep(.monaco-editor),
.template-monaco-editor :deep(.monaco-editor-background),
.template-monaco-editor :deep(.monaco-editor .margin) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New' !important;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.row-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.row-name-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.builtin-tag {
  flex: 0 0 auto;
  font-size: 11px;
  line-height: 18px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  opacity: 0.85;
}

.active {
  background: rgba(45, 212, 191, 0.12);
}

:global(:root[data-guaxi-theme='dark']) .active {
  background: rgba(97, 175, 239, 0.16);
}

:global(:root[data-guaxi-theme='dark']) .status-pill {
  background: rgba(97, 175, 239, 0.16);
}

@media (max-width: 920px) {
  .header {
    align-items: flex-start;
  }

  .header-main {
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .header-summary {
    max-width: 100%;
    overflow-x: auto;
  }
}
</style>
