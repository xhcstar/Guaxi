//! 语音引擎实例的进程内缓存。
//!
//! 加载一次模型要数秒（kokoro 实测约 3.5s），而有声书一本就是成百上千次调用，
//! 所以引擎必须常驻、按配置键复用；键变化即丢弃重建。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

/// 同一类引擎最多同时缓存几份。模型一份几百 MB，不能无限堆着；
/// 实际上本应用的配置是全局的，超过上限基本只可能是用户切了模型。
const MAX_CACHED_ENGINES: usize = 2;

/// 被缓存的引擎实例。
///
/// `inner` 用互斥锁串行化调用：sherpa-onnx 没有声明实例线程安全，
/// 其音素化 / lexicon 前端还带可变状态，并发调用同一实例有数据竞争风险。
/// 单实例内部靠 `num_threads` 吃多核，不需要靠并发调用堆吞吐。
pub struct CachedEngine<T> {
    inner: Mutex<T>,
}

impl<T> CachedEngine<T> {
    /// 取得底层引擎。**不要跨 await 持有**：这是同步锁，且调用本身是阻塞的。
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock()
    }
}

pub struct EngineCache<T> {
    slots: Mutex<HashMap<u64, Arc<CachedEngine<T>>>>,
}

impl<T> EngineCache<T> {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// 取键对应的实例，没有就用 `create` 建一个。
    ///
    /// `create` 在持有槽位锁时执行——它很慢（要读几百 MB 模型），但这正好保证
    /// 并发的首次调用只会加载一次，不会几路同时各载一份。
    pub fn get_or_create<F>(&self, key: u64, create: F) -> Result<Arc<CachedEngine<T>>, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        let mut slots = self.slots.lock();
        if let Some(existing) = slots.get(&key) {
            return Ok(existing.clone());
        }

        let engine = create()?;

        if slots.len() >= MAX_CACHED_ENGINES {
            slots.clear();
        }
        let entry = Arc::new(CachedEngine {
            inner: Mutex::new(engine),
        });
        slots.insert(key, entry.clone());
        Ok(entry)
    }
}
