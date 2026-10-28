// Git 变更列表的树形分组。
//
// VS Code 的 SCM 面板默认按目录折叠；这里把扁平的文件列表展开成
// `[{type:'folder'} | {type:'file'}]` 的行序列，交给模板直接 v-for，
// 避免为一个很小的树写递归组件。折叠状态由调用方用 Set 维护。

const normalizePath = (value) => String(value || '').replace(/\\/g, '/')

const baseName = (value) => {
  const normalized = normalizePath(value)
  const index = normalized.lastIndexOf('/')
  return index >= 0 ? normalized.slice(index + 1) : normalized
}

const countFiles = (node) => {
  let count = node.files.length
  for (const child of node.folders.values()) {
    count += countFiles(child)
  }
  return count
}

const createNode = (name, path) => ({
  name,
  path,
  folders: new Map(),
  files: [],
})

export function buildGitFileTreeRows(files, { collapsed, mode = 'tree', prefix = '' } = {}) {
  const list = Array.isArray(files) ? files : []

  if (mode === 'list') {
    return list
      .slice()
      .sort((a, b) => normalizePath(a?.path).localeCompare(normalizePath(b?.path)))
      .map((file) => ({
        type: 'file',
        key: `${prefix}:file:${normalizePath(file?.path)}:${file?.staged ? 'staged' : 'work'}`,
        file,
        depth: 0,
      }))
  }

  const root = createNode('', '')

  for (const file of list) {
    const segments = normalizePath(file?.path).split('/').filter(Boolean)
    if (!segments.length) continue

    let node = root
    for (let index = 0; index < segments.length - 1; index += 1) {
      const name = segments[index]
      let child = node.folders.get(name)
      if (!child) {
        child = createNode(name, segments.slice(0, index + 1).join('/'))
        node.folders.set(name, child)
      }
      node = child
    }
    node.files.push(file)
  }

  const rows = []
  const walk = (node, depth) => {
    const folders = [...node.folders.values()].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
    )
    for (const folder of folders) {
      rows.push({
        type: 'folder',
        key: `${prefix}:folder:${folder.path}`,
        path: folder.path,
        name: folder.name,
        depth,
        count: countFiles(folder),
      })
      if (!collapsed?.has(folder.path)) {
        walk(folder, depth + 1)
      }
    }

    const filesInFolder = node.files
      .slice()
      .sort((a, b) =>
        baseName(a?.path).localeCompare(baseName(b?.path), undefined, { sensitivity: 'base' })
      )
    for (const file of filesInFolder) {
      rows.push({
        type: 'file',
        key: `${prefix}:file:${normalizePath(file?.path)}:${file?.staged ? 'staged' : 'work'}`,
        file,
        depth,
      })
    }
  }

  walk(root, 0)
  return rows
}
