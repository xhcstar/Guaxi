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
        <div class="header-fields">
          <label class="header-field">
            <span>名称</span>
            <n-input
              v-model:value="editName"
              class="header-field-input"
              size="small"
              :disabled="loading"
              placeholder="模板名称"
            />
          </label>
          <label class="header-field">
            <span>类型</span>
            <n-input
              v-model:value="editType"
              class="header-field-input"
              size="small"
              :disabled="loading"
              placeholder="模板分类"
            />
          </label>
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
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button
                size="small"
                circle
                :disabled="loading || !editContent"
                aria-label="插入当前模板内容到主编辑器光标处"
                @click="insertCurrentTemplateToEditor"
              >
                <template #icon>
                  <Icon :icon="sendIcon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            插入当前内容到主编辑器光标处
          </n-tooltip>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button
                size="small"
                circle
                type="primary"
                :disabled="loading || !editName"
                aria-label="保存当前模板"
                @click="save"
              >
                <template #icon>
                  <Icon :icon="saveIcon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            保存当前模板
          </n-tooltip>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button
                size="small"
                circle
                tertiary
                type="error"
                :disabled="loading || !selectedName"
                aria-label="删除当前自定义模板"
                @click="remove"
              >
                <template #icon>
                  <Icon :icon="trash2Icon" width="16" height="16" />
                </template>
              </n-button>
            </template>
            删除当前自定义模板
          </n-tooltip>
        </div>
      </div>

      <div
        ref="mainRef"
        class="main"
        :class="{ 'is-resizing-sidebar': sidebarResizing }"
        :style="{ gridTemplateColumns: `${sidebarWidth}px 6px minmax(0, 1fr)` }"
      >
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
        <div
          class="sidebar-resizer"
          title="拖动调整模板列表宽度"
          role="separator"
          aria-orientation="vertical"
          @pointerdown="startSidebarResize"
        >
          <span></span>
        </div>

        <div class="right">
          <div ref="contentEditorContainer" class="template-monaco-editor"></div>
          <GuaxiAuxEditorStatusBar :editor-instance="contentEditorInstance" language="markdown" />
        </div>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { NButton, NCollapse, NCollapseItem, NInput, NList, NListItem, NTooltip, createDiscreteApi } from 'naive-ui'
import { Icon } from '@iconify/vue'
import * as monaco from 'monaco-editor'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import GuaxiAuxEditorStatusBar from './GuaxiAuxEditorStatusBar.vue'
import { attachSharedEditorFeatures, resolveSharedEditorDisplayOptions, startSharedLsp } from '../../monaco/editorFeatures'
import { GUAXI_MONACO_SEMANTIC_TOKEN_COLORS, GUAXI_MONACO_THEME_RULES } from '../../monaco/guaxiMonacoThemes'
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
const mainRef = ref(null)
const sidebarWidth = ref(360)
const sidebarResizing = ref(false)

const TEMPLATE_EDITOR_FONT_FAMILY = "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'"
const TEMPLATE_EDITOR_FONT_SIZE = 18
const TEMPLATE_MANAGER_SIDEBAR_WIDTH_KEY = 'guaxi.template_manager.sidebar_width'
const SIDEBAR_MIN_WIDTH = 240
const SIDEBAR_MAX_WIDTH = 720
let contentEditor = null
let contentModel = null
// 供底部状态栏使用：shallowRef 不代理 Monaco 实例，只用来触发一次渲染。
const contentEditorInstance = shallowRef(null)
let featuresDisposable = null
let suppressEditorChange = false
let themeObserver = null
let sidebarResizeStartX = 0
let sidebarResizeStartWidth = 360

const PYTHON_CELL_TEMPLATE_GROUP = '探索者 / Python Cell'
const py = String.raw

// 设计说明：
// 1. 模板管理器只维护可读、可复制的 Python 代码正文，不再写入 Python Cell 的内部存储哨兵标记。
// 2. 每个模板开头保留用途、依赖和改造点说明，插入到主编辑器或 Python Cell 后都可以直接阅读和二次修改。
// 3. 模板按数据、网络、文本、多媒体、二进制、自动化、机器学习等常见工作流拆分，避免用户在一个超大模板里反复删改。
const buildPythonCellTemplate = (name, category, code) => ({
  name: `Python Cell｜${name}`,
  templateType: `${PYTHON_CELL_TEMPLATE_GROUP} / ${category}`,
  content: `# 用途：${name}\n# 使用说明：按你的真实路径、字段名、接口地址和依赖库安装情况修改 TODO 位置。\n# 建议：先小样本运行，确认输出正确后再处理全量数据。\n\n${String(code || '').trim()}\n`
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
`),
  buildPythonCellTemplate('数值计算：NumPy 向量化 / 统计量', '数值计算', py`
import numpy as np

# TODO: 替换为你的原始数组或 DataFrame 列。
values = np.array([12.5, 18.0, 21.3, 19.8, 30.2], dtype=float)

normalized = (values - values.mean()) / values.std(ddof=1)
rolling_mean = np.convolve(values, np.ones(3) / 3, mode="valid")

print("count:", values.size)
print("mean:", values.mean())
print("median:", np.median(values))
print("p95:", np.percentile(values, 95))
print("z-score:", normalized)
print("rolling_mean:", rolling_mean)
`),
  buildPythonCellTemplate('统计分析：SciPy 假设检验', '统计分析', py`
import numpy as np
from scipy import stats

# TODO: 替换为两组实验 / 对照样本。
control = np.array([10.1, 9.8, 10.5, 10.0, 9.9])
experiment = np.array([11.2, 10.9, 11.5, 11.1, 10.8])

t_stat, p_value = stats.ttest_ind(experiment, control, equal_var=False)
effect_size = (experiment.mean() - control.mean()) / np.sqrt((experiment.var(ddof=1) + control.var(ddof=1)) / 2)

print(f"control_mean={control.mean():.4f}")
print(f"experiment_mean={experiment.mean():.4f}")
print(f"t={t_stat:.4f}, p={p_value:.6f}, cohen_d={effect_size:.4f}")
`),
  buildPythonCellTemplate('机器学习：scikit-learn 训练评估', '机器学习', py`
import pandas as pd
from sklearn.compose import ColumnTransformer
from sklearn.ensemble import RandomForestClassifier
from sklearn.impute import SimpleImputer
from sklearn.metrics import classification_report, confusion_matrix
from sklearn.model_selection import train_test_split
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder, StandardScaler

df = pd.read_csv("data/train.csv", encoding="utf-8")
target_col = "target"  # TODO: 替换目标列。

X = df.drop(columns=[target_col])
y = df[target_col]
numeric_cols = X.select_dtypes(include="number").columns.tolist()
category_cols = X.select_dtypes(exclude="number").columns.tolist()

preprocess = ColumnTransformer([
    ("num", Pipeline([("imputer", SimpleImputer(strategy="median")), ("scaler", StandardScaler())]), numeric_cols),
    ("cat", Pipeline([("imputer", SimpleImputer(strategy="most_frequent")), ("onehot", OneHotEncoder(handle_unknown="ignore"))]), category_cols),
])

model = Pipeline([
    ("preprocess", preprocess),
    ("classifier", RandomForestClassifier(n_estimators=200, random_state=42)),
])

X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)
model.fit(X_train, y_train)
pred = model.predict(X_test)

print(classification_report(y_test, pred))
print(confusion_matrix(y_test, pred))
`),
  buildPythonCellTemplate('机器学习：模型保存 / 加载', '机器学习', py`
from pathlib import Path
import joblib

model_path = Path("output/model.joblib")
model_path.parent.mkdir(parents=True, exist_ok=True)

# TODO: 将 model 替换为你训练好的 sklearn Pipeline / 模型对象。
joblib.dump(model, model_path)
print(f"模型已保存: {model_path.resolve()}")

loaded_model = joblib.load(model_path)
# pred = loaded_model.predict(new_data)
`),
  buildPythonCellTemplate('深度学习：PyTorch 最小训练循环', '机器学习', py`
import torch
from torch import nn
from torch.utils.data import DataLoader, TensorDataset

device = "cuda" if torch.cuda.is_available() else "cpu"

# TODO: 替换为真实特征和标签。
X = torch.randn(128, 10)
y = (X.sum(dim=1) > 0).long()
loader = DataLoader(TensorDataset(X, y), batch_size=32, shuffle=True)

model = nn.Sequential(nn.Linear(10, 32), nn.ReLU(), nn.Linear(32, 2)).to(device)
loss_fn = nn.CrossEntropyLoss()
optimizer = torch.optim.AdamW(model.parameters(), lr=1e-3)

for epoch in range(5):
    model.train()
    total_loss = 0.0
    for xb, yb in loader:
        xb, yb = xb.to(device), yb.to(device)
        optimizer.zero_grad()
        loss = loss_fn(model(xb), yb)
        loss.backward()
        optimizer.step()
        total_loss += loss.item() * xb.size(0)
    print(f"epoch={epoch + 1}, loss={total_loss / len(loader.dataset):.4f}")
`),
  buildPythonCellTemplate('可视化：Seaborn 统计图', '绘图', py`
import pandas as pd
import seaborn as sns
import matplotlib.pyplot as plt

df = pd.read_csv("data/clean.csv", encoding="utf-8")

sns.set_theme(style="whitegrid", font="Microsoft YaHei")
fig, axes = plt.subplots(1, 2, figsize=(12, 4))

sns.boxplot(data=df, x="category", y="amount", ax=axes[0])
axes[0].tick_params(axis="x", rotation=30)
sns.scatterplot(data=df, x="amount", y="score", hue="category", ax=axes[1])

plt.tight_layout()
plt.show()
`),
  buildPythonCellTemplate('可视化：Plotly 交互图导出 HTML', '绘图', py`
from pathlib import Path
import pandas as pd
import plotly.express as px

df = pd.read_csv("data/clean.csv", encoding="utf-8")
fig = px.line(df, x="date", y="amount", color="category", title="趋势分析")

output_path = Path("output/interactive_chart.html")
output_path.parent.mkdir(parents=True, exist_ok=True)
fig.write_html(output_path, include_plotlyjs="cdn")
print(f"交互图已导出: {output_path.resolve()}")
fig.show()
`),
  buildPythonCellTemplate('Excel 自动化：openpyxl 格式化报表', '办公自动化', py`
from pathlib import Path
from openpyxl import load_workbook
from openpyxl.styles import Font, PatternFill
from openpyxl.utils import get_column_letter

path = Path("output/result.xlsx")
wb = load_workbook(path)
ws = wb.active

header_fill = PatternFill("solid", fgColor="1F6FEB")
for cell in ws[1]:
    cell.font = Font(color="FFFFFF", bold=True)
    cell.fill = header_fill

for col in ws.columns:
    max_len = max(len(str(cell.value or "")) for cell in col)
    ws.column_dimensions[get_column_letter(col[0].column)].width = min(max(max_len + 2, 10), 40)

target = path.with_name(f"{path.stem}_formatted.xlsx")
wb.save(target)
print(f"格式化报表已保存: {target.resolve()}")
`),
  buildPythonCellTemplate('配置读取：YAML / TOML / 环境变量', '配置与日志', py`
import os
from pathlib import Path
import tomllib
import yaml

yaml_config = yaml.safe_load(Path("config/app.yaml").read_text(encoding="utf-8"))
with Path("config/pyproject.toml").open("rb") as f:
    toml_config = tomllib.load(f)

api_key = os.getenv("API_KEY", "")
debug = os.getenv("DEBUG", "false").lower() == "true"

print("yaml keys:", yaml_config.keys())
print("toml keys:", toml_config.keys())
print("debug:", debug, "api_key configured:", bool(api_key))
`),
  buildPythonCellTemplate('日志与异常：logging 标准模板', '配置与日志', py`
import logging
from pathlib import Path

log_dir = Path("logs")
log_dir.mkdir(parents=True, exist_ok=True)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s [%(name)s] %(message)s",
    handlers=[
        logging.FileHandler(log_dir / "task.log", encoding="utf-8"),
        logging.StreamHandler(),
    ],
)
logger = logging.getLogger("guaxi.task")

try:
    logger.info("任务开始")
    # TODO: 在这里放置你的处理逻辑。
    result = 1 / 1
    logger.info("任务完成 result=%s", result)
except Exception:
    logger.exception("任务失败")
    raise
`),
  buildPythonCellTemplate('批处理进度：tqdm 扫描处理', '文件处理', py`
from pathlib import Path
from tqdm import tqdm

input_dir = Path("data")
output_dir = Path("output/processed")
output_dir.mkdir(parents=True, exist_ok=True)

files = [p for p in input_dir.rglob("*.txt") if p.is_file()]
for path in tqdm(files, desc="处理文本文件"):
    text = path.read_text(encoding="utf-8", errors="ignore")
    cleaned = "\n".join(line.strip() for line in text.splitlines() if line.strip())
    target = output_dir / path.relative_to(input_dir)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(cleaned, encoding="utf-8")

print(f"处理完成: {len(files)} 个文件")
`),
  buildPythonCellTemplate('并发处理：ThreadPoolExecutor', '性能并发', py`
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

def process_file(path: Path) -> dict:
    text = path.read_text(encoding="utf-8", errors="ignore")
    return {"path": str(path), "chars": len(text), "lines": text.count("\n") + 1}

files = list(Path("data").rglob("*.txt"))
results = []

with ThreadPoolExecutor(max_workers=8) as executor:
    future_map = {executor.submit(process_file, path): path for path in files}
    for future in as_completed(future_map):
        path = future_map[future]
        try:
            results.append(future.result())
        except Exception as exc:
            print(f"处理失败: {path} -> {exc}")

print(results[:10])
`),
  buildPythonCellTemplate('命令行脚本：argparse 入口', '自动化脚本', py`
import argparse
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description="批量处理文件")
    parser.add_argument("input", type=Path, help="输入目录或文件")
    parser.add_argument("-o", "--output", type=Path, default=Path("output"), help="输出目录")
    parser.add_argument("--dry-run", action="store_true", help="只打印计划，不实际写入")
    args = parser.parse_args()

    print("input:", args.input)
    print("output:", args.output)
    print("dry_run:", args.dry_run)
    # TODO: 在这里调用你的处理函数。

if __name__ == "__main__":
    main()
`),
  buildPythonCellTemplate('数据校验：Pydantic Schema', '数据校验', py`
from datetime import datetime
from pydantic import BaseModel, Field, ValidationError

class EventRecord(BaseModel):
    user_id: str = Field(min_length=1)
    event_name: str
    amount: float = 0
    event_time: datetime

raw_records = [
    {"user_id": "u001", "event_name": "purchase", "amount": 19.9, "event_time": "2026-01-01T12:00:00"},
]

valid_records = []
for item in raw_records:
    try:
        valid_records.append(EventRecord.model_validate(item))
    except ValidationError as exc:
        print("校验失败:", item, exc)

print(valid_records)
`),
  buildPythonCellTemplate('API 请求：requests 会话 / 重试', '网络抓取', py`
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

session = requests.Session()
retry = Retry(total=3, backoff_factor=0.5, status_forcelist=[429, 500, 502, 503, 504])
session.mount("https://", HTTPAdapter(max_retries=retry))
session.headers.update({"User-Agent": "GuaxiTemplate/1.0"})

url = "https://api.example.com/items"
response = session.get(url, params={"page": 1}, timeout=20)
response.raise_for_status()

payload = response.json()
print(payload if isinstance(payload, dict) else payload[:3])
`),
  buildPythonCellTemplate('HTML 解析：BeautifulSoup 表格提取', '网络抓取', py`
import pandas as pd
import requests
from bs4 import BeautifulSoup

url = "https://example.com"
html = requests.get(url, timeout=20, headers={"User-Agent": "GuaxiTemplate/1.0"}).text
soup = BeautifulSoup(html, "html.parser")

rows = []
for tr in soup.select("table tr"):
    cells = [cell.get_text(" ", strip=True) for cell in tr.select("th,td")]
    if cells:
        rows.append(cells)

df = pd.DataFrame(rows[1:], columns=rows[0] if rows else None)
df.head()
`),
  buildPythonCellTemplate('PDF 文本提取：pypdf', '文档处理', py`
from pathlib import Path
from pypdf import PdfReader

pdf_path = Path("data/sample.pdf")
reader = PdfReader(str(pdf_path))

pages = []
for index, page in enumerate(reader.pages, start=1):
    text = page.extract_text() or ""
    pages.append({"page": index, "text": text})

output_path = Path("output/pdf_text.txt")
output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text("\n\n".join(f"## Page {p['page']}\n{p['text']}" for p in pages), encoding="utf-8")
print(f"提取页数: {len(pages)}, 输出: {output_path.resolve()}")
`),
  buildPythonCellTemplate('Word 文档生成：python-docx', '文档处理', py`
from pathlib import Path
from docx import Document

doc = Document()
doc.add_heading("自动生成报告", level=1)
doc.add_paragraph("这里写报告摘要。")

table = doc.add_table(rows=1, cols=3)
header = table.rows[0].cells
header[0].text = "指标"
header[1].text = "数值"
header[2].text = "说明"

for metric, value, note in [("样本数", "100", "示例"), ("准确率", "95%", "示例")]:
    cells = table.add_row().cells
    cells[0].text = metric
    cells[1].text = value
    cells[2].text = note

output_path = Path("output/report.docx")
output_path.parent.mkdir(parents=True, exist_ok=True)
doc.save(output_path)
print(f"Word 报告已保存: {output_path.resolve()}")
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
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数值计算`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 统计分析`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 机器学习`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 绘图`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据导出`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 网络抓取`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 文本处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 图片视频处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 二进制文件处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 文件处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 文档处理`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 办公自动化`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 配置与日志`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 性能并发`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 自动化脚本`,
  `${PYTHON_CELL_TEMPLATE_GROUP} / 数据校验`,
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
    // 语法色板复用主编辑器那一份：设置页切换「代码配色主题」后，
    // 这里通过 attachSharedEditorFeatures 的 ensureThemes 重注册，颜色跟着变。
    rules: GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
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
    rules: GUAXI_MONACO_THEME_RULES.dark,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark,
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

const clampSidebarWidth = (value) => {
  const mainWidth = mainRef.value instanceof HTMLElement ? mainRef.value.clientWidth : window.innerWidth
  const dynamicMax = Math.max(SIDEBAR_MIN_WIDTH, Math.min(SIDEBAR_MAX_WIDTH, mainWidth - 420))
  return Math.round(Math.max(SIDEBAR_MIN_WIDTH, Math.min(dynamicMax, Number(value) || 360)))
}

const loadSidebarWidth = () => {
  try {
    const saved = Number(localStorage.getItem(TEMPLATE_MANAGER_SIDEBAR_WIDTH_KEY) || '')
    if (Number.isFinite(saved) && saved > 0) {
      sidebarWidth.value = clampSidebarWidth(saved)
    }
  } catch (error) {
    logger.debug('[TemplateManager] load sidebar width failed', error)
  }
}

const persistSidebarWidth = () => {
  try {
    localStorage.setItem(TEMPLATE_MANAGER_SIDEBAR_WIDTH_KEY, String(sidebarWidth.value))
  } catch (error) {
    logger.debug('[TemplateManager] persist sidebar width failed', error)
  }
}

const handleSidebarResizeMove = (event) => {
  if (!sidebarResizing.value) return
  const delta = Number(event.clientX || 0) - sidebarResizeStartX
  sidebarWidth.value = clampSidebarWidth(sidebarResizeStartWidth + delta)
  contentEditor?.layout?.()
}

const stopSidebarResize = () => {
  if (!sidebarResizing.value) return
  sidebarResizing.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('pointermove', handleSidebarResizeMove)
  window.removeEventListener('pointerup', stopSidebarResize)
  window.removeEventListener('pointercancel', stopSidebarResize)
  persistSidebarWidth()
  contentEditor?.layout?.()
  logger.info('[TemplateManager] sidebar resize finished', {
    width: sidebarWidth.value,
  })
}

const startSidebarResize = (event) => {
  sidebarResizing.value = true
  sidebarResizeStartX = Number(event.clientX || 0)
  sidebarResizeStartWidth = sidebarWidth.value
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('pointermove', handleSidebarResizeMove)
  window.addEventListener('pointerup', stopSidebarResize)
  window.addEventListener('pointercancel', stopSidebarResize)
  event.preventDefault?.()
  logger.info('[TemplateManager] sidebar resize started', {
    width: sidebarWidth.value,
  })
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

// 模板内容不是磁盘文件，但 LSP 需要 file:// 文档，而且扩展名决定语言服务器是否接管
// （pyright 只分析 .py，markdown 服务只分析 .md），所以按语言各用一个虚拟文件。
const buildTemplateVirtualUri = (language) => {
  const ext = language === 'python' ? 'py' : 'md'
  return monaco.Uri.parse(`file:///guaxi-template-manager/template-${Date.now()}.${ext}`)
}

// Python Cell 模板正文是纯 Python，若按 markdown 打开就永远拿不到 Python 语义补全。
const PYTHON_TEMPLATE_TYPE_RE = /python\s*cell|python\s*模板|^python$/i

const resolveTemplateLanguage = () => {
  const type = String(editType.value || '').trim()
  const name = String(editName.value || selectedName.value || '').trim()
  return (PYTHON_TEMPLATE_TYPE_RE.test(type) || PYTHON_TEMPLATE_TYPE_RE.test(name)) ? 'python' : 'markdown'
}

let currentEditorLanguage = 'markdown'

const handleContentModelChange = () => {
  if (suppressEditorChange) return
  editContent.value = contentModel?.getValue() || ''
}

const createTemplateModel = (language, content) => {
  const nextModel = monaco.editor.createModel(
    String(content || ''),
    language,
    buildTemplateVirtualUri(language)
  )
  nextModel.updateOptions({ tabSize: language === 'python' ? 4 : 2, insertSpaces: true })
  nextModel.onDidChangeContent(handleContentModelChange)
  return nextModel
}

const applyTemplateEditorLanguage = (reason = 'manual') => {
  if (!contentEditor || !contentModel) return
  const next = resolveTemplateLanguage()
  if (next === currentEditorLanguage) return

  const prev = currentEditorLanguage
  const content = contentModel.getValue()
  const previousModel = contentModel
  contentModel = createTemplateModel(next, content)
  contentEditor.setModel(contentModel)
  previousModel?.dispose?.()
  currentEditorLanguage = next

  // 补全提供方按语言注册；换语言后为目标语言重新起一次 LSP，Python 模板才拿得到语义补全。
  void startSharedLsp(contentEditor, next, contentModel.uri.toString(), {
    requestSource: 'template-manager',
    documentUri: contentModel.uri.toString(),
  })
  logger.info('[TemplateManager] editor language switched', { from: prev, to: next, reason })
}

const initContentEditor = () => {
  if (contentEditor || !contentEditorContainer.value) return

  defineTemplateEditorThemes()
  const display = resolveSharedEditorDisplayOptions()
  const initialLanguage = resolveTemplateLanguage()
  contentModel = createTemplateModel(initialLanguage, editContent.value)
  currentEditorLanguage = initialLanguage
  contentEditor = monaco.editor.create(contentEditorContainer.value, {
    model: contentModel,
    automaticLayout: true,
    // 右键菜单由 editorFeatures 提供，关掉 Monaco 自带菜单避免双弹。
    contextmenu: false,
    minimap: display.minimap,
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    lineNumbers: 'on',
    renderWhitespace: 'selection',
    tabSize: 2,
    insertSpaces: true,
    fontFamily: display.fontFamily,
    fontSize: display.fontSize,
    lineHeight: display.lineHeight,
    readOnly: loading.value,
    theme: getTemplateEditorTheme(),
  })

  // 与主编辑器一致的基础能力：Markdown 片段、右键菜单、格式化、LSP、主题同步。
  featuresDisposable = attachSharedEditorFeatures({
    monaco,
    editor: contentEditor,
    language: initialLanguage,
    filePath: contentModel.uri.toString(),
    enableLsp: true,
    lspDocumentUri: contentModel.uri.toString(),
    lspRequestSource: 'template-manager',
    ensureThemes: defineTemplateEditorThemes,
    resolveThemeName: (dark) => (dark ? 'guaxi-template-dark' : 'guaxi-template-light'),
  })
  contentEditorInstance.value = contentEditor

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

const insertCurrentTemplateToEditor = async () => {
  const content = String(editContent.value || '')
  if (!content.trim()) {
    logger.info('[TemplateManager] insert current skipped: empty content', {
      name: editName.value,
    })
    message.warning('当前模板内容为空')
    return
  }

  try {
    loading.value = true
    logger.info('[TemplateManager] insert current editor content requested', {
      name: editName.value,
      templateType: editType.value,
      length: content.length,
    })
    await invoke('emit_template_insert', { content })
    message.success('已插入到主编辑器光标位置')
    logger.info('[TemplateManager] inserted current editor content', {
      name: editName.value,
      length: content.length,
    })
  } catch (e) {
    logger.error('插入当前模板内容失败:', e)
    message.error('插入当前模板内容失败')
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

// 切换模板时类型跟着变：Python Cell 模板自动切到 python 语言，才能拿到 LSP 补全。
watch(editType, () => {
  applyTemplateEditorLanguage('type-change')
})

watch(loading, (value) => {
  contentEditor?.updateOptions?.({ readOnly: !!value })
})

onMounted(async () => {
  await nextTick()
  loadSidebarWidth()
  initContentEditor()
  await refresh()
})

onBeforeUnmount(() => {
  stopSidebarResize()
  themeObserver?.disconnect?.()
  themeObserver = null
  featuresDisposable?.()
  featuresDisposable = null
  contentEditorInstance.value = null
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
  padding: 0;
  box-sizing: border-box;
  gap: 0;
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
  min-height: 48px;
  padding: 8px 12px;
  background: var(--guaxi-panel-bg);
  border: 0;
  border-bottom: 1px solid var(--guaxi-border);
  box-shadow: none;
  box-sizing: border-box;
}

.header-main {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 0 1 520px;
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

.header-summary span:not(.status-pill) {
  overflow: hidden;
  text-overflow: ellipsis;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.header-fields {
  min-width: 640px;
  max-width: none;
  display: grid;
  grid-template-columns: minmax(380px, 1.5fr) minmax(260px, 1fr);
  gap: 14px;
  flex: 1 1 820px;
}

.header-field {
  min-width: 0;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  font-weight: 650;
}

.header-field-input {
  min-width: 0;
}

.header-field-input :deep(.n-input) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.header-field-input :deep(.n-input__input-el) {
  text-overflow: clip;
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
  grid-template-rows: minmax(0, 1fr);
  gap: 0;
  overflow: hidden;
  background: var(--guaxi-panel-bg);
}

.left {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
  border: 0;
  border-radius: 0;
  padding: 10px 10px 8px 12px;
  background: var(--guaxi-panel-bg);
  box-shadow: none;
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
  border: 0;
  border-radius: 0;
  padding: 0;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  box-shadow: none;
  display: flex;
  flex-direction: column;
}

.sidebar-resizer {
  position: relative;
  min-width: 6px;
  width: 6px;
  cursor: col-resize;
  background: var(--guaxi-content-bg);
  border-left: 1px solid var(--guaxi-border);
  border-right: 1px solid var(--guaxi-border);
  box-sizing: border-box;
  z-index: 2;
}

.sidebar-resizer span {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 2px;
  width: 2px;
  background: transparent;
  transition: background 0.12s ease;
}

.sidebar-resizer:hover span,
.main.is-resizing-sidebar .sidebar-resizer span {
  background: rgba(45, 212, 191, 0.72);
}

.template-monaco-editor {
  flex: 1;
  min-height: 0;
  height: 100%;
  width: 100%;
  box-sizing: border-box;
  overflow: hidden;
  border: 0;
  border-radius: 0;
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

:global(:root[data-guaxi-theme='dark']) .sidebar-resizer:hover span,
:global(:root[data-guaxi-theme='dark']) .main.is-resizing-sidebar .sidebar-resizer span {
  background: rgba(97, 175, 239, 0.78);
}

@media (max-width: 920px) {
  .header {
    align-items: flex-start;
    flex-wrap: wrap;
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

  .header-fields {
    order: 3;
    flex-basis: 100%;
    max-width: none;
    grid-template-columns: 1fr;
  }
}
</style>
