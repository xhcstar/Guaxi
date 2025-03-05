# 数据库设计

## 导入的文件信息

|id|文件名|文件类型|入库时间|文件大小|文件路径|所属库|文件描述|文件标签|转换html文件名|转换markdown文件名|
|-|-|-|-|-|-|-|-|-|-|-|
|id|title|type|create_time|size|path|library|description|tags|html_file_name|markdown_file_name|

为了解决导入文件信息的数据库设计，我们需要考虑如何存储关于文件的详细信息，包括文件的基本信息、标签和文件的转换信息。以下是设计数据库的详细方案：

### 1. **数据库设计**

#### 1.1 **导入文件表（`imported_files`）**

记录导入文件的所有基本信息，包括文件名、文件类型、文件大小、文件路径、所属库、文件描述等。

```sql
CREATE TABLE imported_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,         -- 文件的唯一标识符
    title TEXT NOT NULL,                          -- 文件名
    type TEXT NOT NULL,                           -- 文件类型 (如：pdf, docx, txt等)
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 入库时间
    size INTEGER,                                 -- 文件大小（单位：字节）
    path TEXT NOT NULL,                           -- 文件路径
    library TEXT,                                 -- 所属库
    description TEXT,                             -- 文件描述
    tags TEXT,                                    -- 文件标签，多个标签用逗号分隔
    html_file_name TEXT,                          -- 转换后的HTML文件名
    markdown_file_name TEXT                       -- 转换后的Markdown文件名
);
```

### 2. **表字段说明**

- **id**: 唯一标识每个文件，自动递增。
- **title**: 文件的名称。
- **type**: 文件的类型，例如“pdf”、“docx”等。
- **create_time**: 文件的入库时间，默认为当前时间。
- **size**: 文件的大小（单位：字节）。
- **path**: 文件在本地的存储路径。
- **library**: 文件所属的库。
- **description**: 文件的简短描述。
- **tags**: 文件的标签，多个标签用逗号分隔（可以考虑进一步设计标签表，如果需要支持复杂的多对多关系）。
- **html_file_name**: 转换后的HTML文件名。
- **markdown_file_name**: 转换后的Markdown文件名。

### 3. **设计示例数据**

假设有以下数据：

| id  | title     | type  | create_time          | size  | path              | library  | description     | tags        | html_file_name  | markdown_file_name |
| --- | --------- | ----- | -------------------- | ----- | ----------------- | -------- | --------------- | ---------- | --------------- | ------------------ |
| 1   | 文件1.pdf | pdf   | 2025-03-06 08:00:00  | 2048  | /path/to/file1.pdf | Library1 | 这是文件1       | 标签1,标签2 | file1.html      | file1.md           |
| 2   | 文件2.docx| docx  | 2025-03-06 09:00:00  | 4096  | /path/to/file2.docx| Library2 | 这是文件2       | 标签3      | file2.html      | file2.md           |

### 4. **额外的设计考虑**

#### 4.1 **标签表设计**

如果需要处理标签信息的多对多关系，可以使用标签表和文件标签关联表来管理文件与标签的关系，而不是在`imported_files`表中直接存储标签。以下是如何实现标签多对多关系的设计。

#### 4.2 **标签表（`tags`）**

记录标签信息。

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE -- 标签名称，确保唯一
);
```

#### 4.3 **文件标签关联表（`file_tags`）**

该表用来记录文件与标签的多对多关系。

```sql
CREATE TABLE file_tags (
    file_id INTEGER,                     -- 文件ID
    tag_id INTEGER,                      -- 标签ID
    PRIMARY KEY (file_id, tag_id),       -- 联合主键，确保文件与标签的组合唯一
    FOREIGN KEY(file_id) REFERENCES imported_files(id), -- 外键关联文件
    FOREIGN KEY(tag_id) REFERENCES tags(id)            -- 外键关联标签
);
```

这样，文件和标签之间的关系就可以通过**多对多**关系来管理，每个文件可以有多个标签，而每个标签也可以关联多个文件。

### 5. **查询示例**

#### 5.1 **查询文件的标签**

```sql
SELECT tags.name
FROM tags
JOIN file_tags ON tags.id = file_tags.tag_id
WHERE file_tags.file_id = 1; -- 查询文件1的所有标签
```

#### 5.2 **查询具有特定标签的文件**

```sql
SELECT imported_files.title, imported_files.description
FROM imported_files
JOIN file_tags ON imported_files.id = file_tags.file_id
JOIN tags ON file_tags.tag_id = tags.id
WHERE tags.name = '标签1'; -- 查询所有标签为“标签1”的文件
```

#### 5.3 **查询文件及其转换后的HTML和Markdown文件**

```sql
SELECT title, html_file_name, markdown_file_name
FROM imported_files
WHERE type = 'pdf'; -- 查询所有类型为“pdf”的文件及其转换后的文件名
```

### 6. **前端展示设计**

- **文件页面**：显示文件的标题、类型、入库时间、标签等基本信息。标签可以显示为可以点击的链接，点击后查看所有带有该标签的文件。
- **标签筛选**：支持用户根据标签筛选文件，展示所有具有特定标签的文件。
- **文件管理**：支持用户管理文件的标签，添加或删除标签，并确保没有重复标签。

### 7. **注意事项**

- 通过 **联合主键**（`PRIMARY KEY (file_id, tag_id)`）确保文件与标签的组合唯一，避免重复。
- 标签的存储方式可以根据需求选择：简单的用逗号分隔存储标签，或者通过多对多关系表来更精确地控制。
- 文件的转换信息（HTML和Markdown文件）可以存储文件名，但如果转换过程涉及复杂的配置或状态，可能需要额外的表来记录这些信息。

这样设计能够有效地管理导入的文件信息，并为将来的扩展（如文件标签、转换文件的格式、文件管理等）提供良好的基础。

## 记录的笔记信息

|id|笔记标题|笔记描述|笔记类型|笔记标签|笔记创建时间|笔记修改时间|笔记所属库|笔记所属文件|笔记所属笔记的父级|笔记所属笔记的子级|
|-|-|-|-|-|-|-|-|-|-|-|
|id|title|description|type|tags|create_time|update_time|library|file|parent_note|child_notes|

如果需要在数据库中记录笔记之间的关联关系（除了父子关系以外），比如笔记间的链接、引用或其他类型的关系，可以通过添加一个 **笔记关联表** 来实现。这将允许用户在笔记之间创建灵活的关系，如交叉引用、引用等。

### 1. **表设计更新**

我们需要增加一个新的表来存储笔记之间的关联关系。

#### 1.1 **笔记关联表（note_associations）**

该表用来存储笔记之间的任意关联关系，关联关系的类型（例如“引用”或“链接”）可以作为表的字段记录。

```sql
CREATE TABLE note_associations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id_1 INTEGER NOT NULL,              -- 笔记1的ID
    note_id_2 INTEGER NOT NULL,              -- 笔记2的ID
    relationship_type TEXT NOT NULL,         -- 关系类型（如：引用、链接等）
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 关联创建时间
    FOREIGN KEY(note_id_1) REFERENCES notes(id),   -- 外键，指向笔记1
    FOREIGN KEY(note_id_2) REFERENCES notes(id)    -- 外键，指向笔记2
);
```

### 2. **数据库表结构**

#### 2.1 **笔记表（notes）**

保持不变，记录笔记的基本信息。

```sql
CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,              -- 笔记标题
    description TEXT,                 -- 笔记描述
    type TEXT,                        -- 笔记类型 (例如：文本，图片，代码等)
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 笔记创建时间
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 笔记修改时间
    library TEXT,                     -- 笔记所属库
    file TEXT,                        -- 笔记所属文件
    parent_note INTEGER,              -- 父笔记ID，NULL表示没有父笔记
    FOREIGN KEY(parent_note) REFERENCES notes(id)  -- 外键关联到父笔记
);
```

#### 2.2 **标签表（tags）**

记录标签信息。

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE         -- 标签名称
);
```

#### 2.3 **笔记标签关联表（note_tags）**

实现笔记与标签之间的多对多关系。

```sql
CREATE TABLE note_tags (
    note_id INTEGER,                 -- 笔记ID
    tag_id INTEGER,                  -- 标签ID
    PRIMARY KEY (note_id, tag_id),   -- 联合主键，确保每个笔记只能与一个标签多次关联
    FOREIGN KEY(note_id) REFERENCES notes(id),    -- 外键关联笔记
    FOREIGN KEY(tag_id) REFERENCES tags(id)       -- 外键关联标签
);
```

#### 2.4 **笔记父子关系表（note_relationships）**

记录笔记间的父子关系。

```sql
CREATE TABLE note_relationships (
    parent_id INTEGER,               -- 父笔记ID
    child_id INTEGER,                -- 子笔记ID
    PRIMARY KEY (parent_id, child_id), -- 联合主键，确保每个父子关系只有一条记录
    FOREIGN KEY(parent_id) REFERENCES notes(id), -- 外键，指向父笔记
    FOREIGN KEY(child_id) REFERENCES notes(id)   -- 外键，指向子笔记
);
```

#### 2.5 **笔记关联表（note_associations）**

记录笔记之间的关联关系。

```sql
CREATE TABLE note_associations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id_1 INTEGER NOT NULL,              -- 笔记1的ID
    note_id_2 INTEGER NOT NULL,              -- 笔记2的ID
    relationship_type TEXT NOT NULL,         -- 关系类型（如：引用、链接等）
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 关联创建时间
    FOREIGN KEY(note_id_1) REFERENCES notes(id),   -- 外键，指向笔记1
    FOREIGN KEY(note_id_2) REFERENCES notes(id)    -- 外键，指向笔记2
);
```

### 3. **数据示例**

假设有以下数据：

- **笔记 1**（ID = 1）：根笔记，没有父级。
- **笔记 2**（ID = 2）：子笔记，属于笔记 1。
- **笔记 3**（ID = 3）：子笔记，属于笔记 1。
- **笔记 4**（ID = 4）：引用了笔记 1 和 2。

| id  | title     | description | type  | create_time          | update_time          | library | file     | parent_note | child_notes |
| --- | --------- | ----------- | ----- | -------------------- | -------------------- | ------- | -------- | ----------- | ----------- |
| 1   | 笔记 1    | 这是笔记1   | 文本  | 2025-03-06 08:00:00  | 2025-03-06 08:00:00  | Library1| file1.txt| NULL        | 2, 3        |
| 2   | 笔记 2    | 这是笔记2   | 图片  | 2025-03-06 08:05:00  | 2025-03-06 08:05:00  | Library1| file2.jpg| 1           | NULL        |
| 3   | 笔记 3    | 这是笔记3   | 文本  | 2025-03-06 08:10:00  | 2025-03-06 08:10:00  | Library1| file3.txt| 1           | NULL        |
| 4   | 笔记 4    | 这是笔记4   | 文本  | 2025-03-06 08:15:00  | 2025-03-06 08:15:00  | Library1| file4.txt| NULL        | NULL        |

**笔记关联表**（记录关联关系）：

| id  | note_id_1 | note_id_2 | relationship_type | create_time          |
| --- | --------- | --------- | ----------------- | -------------------- |
| 1   | 4         | 1         | 引用              | 2025-03-06 08:15:00  |
| 2   | 4         | 2         | 引用              | 2025-03-06 08:15:00  |

### 4. **查询示例**

#### 4.1 **查询笔记间的关联**

```sql
SELECT * FROM note_associations
WHERE note_id_1 = 4; -- 查询笔记4与其他笔记的关联
```

#### 4.2 **查询笔记的所有引用关系**

```sql
SELECT * FROM note_associations
WHERE relationship_type = '引用';
```

#### 4.3 **查询笔记的父子关系**

```sql
SELECT * FROM notes
WHERE id IN (SELECT child_id FROM note_relationships WHERE parent_id = 1);
```

#### 4.4 **查询某笔记的所有标签**

```sql
SELECT tags.name FROM tags
JOIN note_tags ON tags.id = note_tags.tag_id
WHERE note_tags.note_id = 4;
```

### 5. **前端展示设计**

前端界面展示笔记关联的实现可以通过以下方式：

- **树状结构**：显示笔记的父子关系，类似树形视图展示笔记的层级。
- **标签筛选**：按标签筛选笔记。
- **关联关系**：通过表格或链接展示笔记间的关联，支持用户点击查看关联的笔记。
- **笔记详情**：显示笔记的基本信息，支持查看与其他笔记的关系。

通过这些设计，可以实现一个功能丰富的笔记管理系统，支持记录笔记之间的各种关联（包括父子关系、标签、引用等）。

为了确保每个笔记只能与标签表中的标签多对多关联，并且每个笔记只能拥有一个特定标签的唯一组合，设计数据库时需要使用 **多对多关系表** 来表示笔记与标签之间的关系，并且在 **笔记标签关联表（`note_tags`）** 上创建联合唯一索引，以防止重复标签的关联。

### 1. **更新数据库设计**

#### 1.1 **笔记表（`notes`）**

笔记表保持不变，记录每个笔记的基本信息。

```sql
CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,              -- 笔记标题
    description TEXT,                 -- 笔记描述
    type TEXT,                        -- 笔记类型 (例如：文本，图片，代码等)
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 笔记创建时间
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 笔记修改时间
    library TEXT,                     -- 笔记所属库
    file TEXT,                        -- 笔记所属文件
    parent_note INTEGER,              -- 父笔记ID，NULL表示没有父笔记
    FOREIGN KEY(parent_note) REFERENCES notes(id)  -- 外键关联到父笔记
);
```

#### 1.2 **标签表（`tags`）**

记录标签信息，确保每个标签名称是唯一的。

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE         -- 标签名称，确保唯一
);
```

#### 1.3 **笔记标签关联表（`note_tags`）**

该表实现笔记与标签的多对多关系，并且 **联合唯一索引** 使得每个笔记和标签的组合是唯一的，防止重复标签的关联。

```sql
CREATE TABLE note_tags (
    note_id INTEGER,                 -- 笔记ID
    tag_id INTEGER,                  -- 标签ID
    PRIMARY KEY (note_id, tag_id),   -- 联合主键，确保每个笔记与标签的组合唯一
    FOREIGN KEY(note_id) REFERENCES notes(id),    -- 外键关联笔记
    FOREIGN KEY(tag_id) REFERENCES tags(id)       -- 外键关联标签
);
```

### 2. **数据示例**

假设有以下数据：

- **笔记 1**（ID = 1）：一个文本笔记，关联标签“学习”，“工作”。
- **笔记 2**（ID = 2）：一个图片笔记，关联标签“生活”，“旅行”。
- **笔记 3**（ID = 3）：一个文本笔记，关联标签“学习”，“工作”。

| id  | title     | description  | type  | create_time          | update_time          | library | file     | parent_note |
| --- | --------- | ------------ | ----- | -------------------- | -------------------- | ------- | -------- | ----------- |
| 1   | 笔记 1    | 这是笔记1    | 文本  | 2025-03-06 08:00:00  | 2025-03-06 08:00:00  | Library1| file1.txt| NULL        |
| 2   | 笔记 2    | 这是笔记2    | 图片  | 2025-03-06 08:05:00  | 2025-03-06 08:05:00  | Library1| file2.jpg| NULL        |
| 3   | 笔记 3    | 这是笔记3    | 文本  | 2025-03-06 08:10:00  | 2025-03-06 08:10:00  | Library1| file3.txt| NULL        |

**标签表：**

| id  | name    |
| --- | ------- |
| 1   | 学习    |
| 2   | 工作    |
| 3   | 生活    |
| 4   | 旅行    |

**笔记标签关联表：**

| note_id | tag_id |
| ------- | ------ |
| 1       | 1      |  -- 笔记1 关联 标签 "学习"
| 1       | 2      |  -- 笔记1 关联 标签 "工作"
| 2       | 3      |  -- 笔记2 关联 标签 "生活"
| 2       | 4      |  -- 笔记2 关联 标签 "旅行"
| 3       | 1      |  -- 笔记3 关联 标签 "学习"
| 3       | 2      |  -- 笔记3 关联 标签 "工作"

### 3. **查询示例**

#### 3.1 **查询笔记的所有标签**

```sql
SELECT tags.name
FROM tags
JOIN note_tags ON tags.id = note_tags.tag_id
WHERE note_tags.note_id = 1; -- 查询笔记1的所有标签
```

#### 3.2 **查询具有特定标签的所有笔记**

```sql
SELECT notes.title, notes.description
FROM notes
JOIN note_tags ON notes.id = note_tags.note_id
JOIN tags ON note_tags.tag_id = tags.id
WHERE tags.name = '学习'; -- 查询所有标签为“学习”的笔记
```

#### 3.3 **查询具有多个标签的笔记**

```sql
SELECT notes.title, notes.description
FROM notes
JOIN note_tags ON notes.id = note_tags.note_id
WHERE note_tags.tag_id IN (SELECT id FROM tags WHERE name IN ('学习', '工作'))
GROUP BY notes.id
HAVING COUNT(DISTINCT note_tags.tag_id) = 2; -- 查询同时拥有标签“学习”和“工作”的笔记
```

### 4. **前端展示设计**

- **笔记页面**：显示笔记的标题、描述、创建时间、标签等基本信息。标签显示为可以点击的链接，点击后可以查看所有带有该标签的笔记。
- **标签筛选**：提供标签过滤器，用户可以根据标签筛选笔记，展示所有带有特定标签的笔记。
- **标签管理**：支持用户为笔记添加、删除标签，并且确保标签不会重复添加。

### 5. **注意事项**

- 使用 **联合主键**（`PRIMARY KEY (note_id, tag_id)`）来保证同一个笔记与同一个标签的关联关系是唯一的，防止重复标签的插入。
- 前端可以根据标签的个数、笔记的数量等信息动态生成页面，支持用户轻松进行标签管理、笔记筛选等操作。

通过这种设计，可以保证每个笔记与标签之间的关系清晰、唯一，并且避免了标签重复的情况。
