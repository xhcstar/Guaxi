const fileStateStore = new Map()

const normalizeFileKey = (filePath) => String(filePath || '__guaxi_global__').trim() || '__guaxi_global__'
const normalizeCellKey = (cellId) => String(cellId || '').trim()

const ensureFileState = (filePath) => {
  const key = normalizeFileKey(filePath)
  if (!fileStateStore.has(key)) {
    fileStateStore.set(key, new Map())
  }
  return fileStateStore.get(key)
}

export const getPythonCellSnapshot = (filePath, cellId) => {
  const key = normalizeCellKey(cellId)
  if (!key) return null
  const state = fileStateStore.get(normalizeFileKey(filePath))
  if (!state) return null
  const value = state.get(key)
  return value && typeof value === 'object' ? { ...value } : null
}

export const patchPythonCellSnapshot = (filePath, cellId, patch = {}) => {
  const key = normalizeCellKey(cellId)
  if (!key) return null
  const state = ensureFileState(filePath)
  const prev = state.get(key)
  const next = {
    ...(prev && typeof prev === 'object' ? prev : {}),
    ...(patch && typeof patch === 'object' ? patch : {}),
  }
  state.set(key, next)
  return { ...next }
}

export const deletePythonCellSnapshot = (filePath, cellId) => {
  const key = normalizeCellKey(cellId)
  if (!key) return
  const fileKey = normalizeFileKey(filePath)
  const state = fileStateStore.get(fileKey)
  if (!state) return
  state.delete(key)
  if (!state.size) {
    fileStateStore.delete(fileKey)
  }
}
