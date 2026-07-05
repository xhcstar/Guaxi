// 设计说明：
// 图片附件的 base64 体积很大（一张手机照片 base64 后常有几 MB）。localStorage 只有
// ~5MB 配额（Android WebView 往往更小），把 base64 塞进采集队列会触发
// "Failed to execute 'setItem' on 'Storage': exceeded the quota"，导致导入图片直接失败。
//
// 这里用 IndexedDB 单独存放附件二进制（按 asset.id 存 base64），队列本身只在
// localStorage 保留轻量元数据。IndexedDB 配额远大于 localStorage，可承载多张图片，
// 且重启后仍可按 id 水合回内存队列再同步到 PC。
//
// 所有方法都做了容错：IndexedDB 不可用时静默降级（内存里仍保留 base64，当前会话可正常同步），
// 不因存储层问题打断采集主流程。

const DB_NAME = 'guaxi-capture'
const DB_VERSION = 1
const STORE_NAME = 'assets'

let dbPromise = null

function openDb() {
  if (dbPromise) return dbPromise

  dbPromise = new Promise((resolve, reject) => {
    if (typeof indexedDB === 'undefined') {
      reject(new Error('IndexedDB 不可用'))
      return
    }

    const request = indexedDB.open(DB_NAME, DB_VERSION)
    request.onupgradeneeded = () => {
      const db = request.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME)
      }
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error || new Error('打开 IndexedDB 失败'))
  }).catch((error) => {
    // 打开失败后允许后续调用重试一次；但先把当前 promise 置空避免锁死。
    dbPromise = null
    throw error
  })

  return dbPromise
}

function runTransaction(mode, handler) {
  return openDb().then(
    (db) =>
      new Promise((resolve, reject) => {
        const tx = db.transaction(STORE_NAME, mode)
        const store = tx.objectStore(STORE_NAME)
        let result
        try {
          result = handler(store)
        } catch (error) {
          reject(error)
          return
        }
        tx.oncomplete = () => resolve(result)
        tx.onerror = () => reject(tx.error || new Error('IndexedDB 事务失败'))
        tx.onabort = () => reject(tx.error || new Error('IndexedDB 事务中止'))
      }),
  )
}

export async function putAssetData(id, dataBase64) {
  if (!id || !dataBase64) return false
  try {
    await runTransaction('readwrite', (store) => store.put(dataBase64, id))
    return true
  } catch (error) {
    console.warn('[capture] asset blob persist failed', { id, error })
    return false
  }
}

export async function getAssetData(id) {
  if (!id) return ''
  try {
    return await runTransaction('readonly', (store) => {
      const request = store.get(id)
      return new Promise((resolve, reject) => {
        request.onsuccess = () => resolve(request.result || '')
        request.onerror = () => reject(request.error)
      })
    })
  } catch (error) {
    console.warn('[capture] asset blob read failed', { id, error })
    return ''
  }
}

export async function deleteAssetData(id) {
  if (!id) return
  try {
    await runTransaction('readwrite', (store) => store.delete(id))
  } catch (error) {
    console.warn('[capture] asset blob delete failed', { id, error })
  }
}
