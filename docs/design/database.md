## 接口设计

1. ​获取导入文件列表
​接口：GET /api/imported-files
​参数：无
​返回值：

```json
[
  {
    "id": 1,
    "importFileName": "file.txt",
    "description": "这是一个文件",
    "library": "library1",
    "tags": ["tag1", "tag2"],
    "type": "text",
    "importFileCreateTime": "2023-10-01T12:00:00Z",
    "importFileSize": "1024"
  }
]
```

2. ​添加导入文件
​接口：POST /api/imported-files
​参数：

```json
{
  "importFileName": "file.txt",
  "description": "这是一个文件",
  "library": "library1",
  "tags": ["tag1", "tag2"],
  "type": "text",
  "importFileCreateTime": "2023-10-01T12:00:00Z",
  "importFileSize": "1024"
}
```

​返回值：

```json
{
  "id": 1,
  "message": "文件添加成功"
}
```

3. ​更新导入文件
​接口：PUT /api/imported-files/:id
​参数：

```json
{
  "importFileName": "new_file.txt",
  "description": "更新后的文件描述",
  "library": "library1",
  "tags": ["tag1", "tag3"],
  "type": "text",
  "importFileCreateTime": "2023-10-01T12:00:00Z",
  "importFileSize": "2048"
}
```

​返回值：

```json
{
  "message": "文件更新成功"
}
```

4. ​删除导入文件
​接口：DELETE /api/imported-files/:id
​参数：无
​返回值：

```json
{
  "message": "文件删除成功"
}
```

## 笔记接口设计

1. ​获取笔记列表
​接口：GET /api/notes
​参数：无
​返回值：
```json
[
  {
    "id": 1,
    "description": "这是一个笔记",
    "noteFileName": "note.md",
    "tags": ["tag1", "tag2"],
    "noteFileType": "markdown",
    "fromImportedFile": "file.txt",
    "fromImportedFileLibrary": "library1",
    "noteFileCreateTime": "2023-10-01T12:00:00Z",
    "noteFileUpdateTime": "2023-10-02T12:00:00Z"
  }
]
```
2. ​添加笔记
​接口：POST /api/notes
​参数：
```json
{
  "description": "这是一个笔记",
  "noteFileName": "note.md",
  "tags": ["tag1", "tag2"],
  "noteFileType": "markdown",
  "fromImportedFile": "file.txt",
  "fromImportedFileLibrary": "library1",
  "noteFileCreateTime": "2023-10-01T12:00:00Z",
  "noteFileUpdateTime": "2023-10-02T12:00:00Z"
}
```
​返回值：
```json
{
  "id": 1,
  "message": "笔记添加成功"
}
```
3. ​更新笔记
​接口：PUT /api/notes/:id
​参数：
```json
{
  "description": "更新后的笔记描述",
  "noteFileName": "new_note.md",
  "tags": ["tag1", "tag3"],
  "noteFileType": "markdown",
  "fromImportedFile": "file.txt",
  "fromImportedFileLibrary": "library1",
  "noteFileCreateTime": "2023-10-01T12:00:00Z",
  "noteFileUpdateTime": "2023-10-03T12:00:00Z"
}
```
​返回值：
```json
{
  "message": "笔记更新成功"
}
```
4. ​删除笔记
​接口：DELETE /api/notes/:id
​参数：无
​返回值：
```json
{
  "message": "笔记删除成功"
}
```
