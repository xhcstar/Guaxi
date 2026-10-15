//! 语音引擎实例的进程内缓存。
//!
//! 加载一次模型要数秒（kokoro 实测约 3.5s），而有声书一本就是成百上千次调用，
//! 所以引擎必须常驻、按配置键复用；键变化即丢弃重建。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

/// 全局递增的取用序号，用来判断"最久未用"（比给每个实例再挂一把时间锁便宜）。
static USE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 同一类引擎最多同时缓存几份。模型一份几百 MB，不能无限堆着。
///
/// 取 4 而不是更小：不同**用途**现在会刻意用不同实例（见 `TtsEngineConfig::instance_tag`：
/// 逐句朗读单独一份，划词朗读与有声书共用一份）。上限太小会让它们互相把对方挤掉，
/// 而一次重建就是几百 MB 读盘加数秒卡顿——那正是朗读最怕的中途停顿。
const MAX_CACHED_ENGINES: usize = 4;

/// 被缓存的引擎实例。
///
/// `inner` 用互斥锁串行化调用：sherpa-onnx 没有声明实例线程安全，
/// 其音素化 / lexicon 前端还带可变状态，并发调用同一实例有数据竞争风险。
/// 单实例内部靠 `num_threads` 吃多核，不需要靠并发调用堆吞吐。
pub struct CachedEngine<T> {
    inner: Mutex<T>,
    /// 最近一次被取用的序号（越大越新）。
    use_seq: AtomicU64,
}

impl<T> CachedEngine<T> {
    /// 取得底层引擎。**不要跨 await 持有**：这是同步锁，且调用本身是阻塞的。
    ///
    /// 取用时刷新"最近使用"序号：淘汰挑的就是序号最小的那个。
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.use_seq
            .store(USE_SEQ.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
        self.inner.lock()
    }

    fn last_used(&self) -> u64 {
        self.use_seq.load(Ordering::Relaxed)
    }

    /// 当前是否空闲（没有调用正持有它）。淘汰时用来避开正在跑推理的实例。
    fn is_idle(&self) -> bool {
        self.inner.try_lock().is_some()
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

        // 先腾位置、再加载。这里以前是 `slots.clear()` 全清：只要另一个用途加载一次引擎，
        // 正在朗读的会话就会被整批清掉，下一句再重建几百 MB 模型——表现为朗读中途长时间卡死。
        if slots.len() >= MAX_CACHED_ENGINES {
            evict_one(&mut slots);
        }

        let engine = create()?;

        let entry = Arc::new(CachedEngine {
            inner: Mutex::new(engine),
            use_seq: AtomicU64::new(USE_SEQ.fetch_add(1, Ordering::Relaxed)),
        });
        slots.insert(key, entry.clone());
        Ok(entry)
    }

    /// 主动卸载一个实例（会话结束时回收专用模型用）。返回是否真的移除了。
    ///
    /// 若该实例正被调用，移除依然安全：调用方持有 `Arc`，用完自然回收；
    /// 只是下次取值会重建。
    pub fn release(&self, key: u64) -> bool {
        self.slots.lock().remove(&key).is_some()
    }
}

/// 淘汰一个槽位：优先"最久未用且当前空闲"的；若都在用，才退化为最久未用的那个。
fn evict_one<T>(slots: &mut HashMap<u64, Arc<CachedEngine<T>>>) {
    let mut idle_victim: Option<(u64, u64)> = None;
    let mut any_victim: Option<(u64, u64)> = None;
    for (key, engine) in slots.iter() {
        let seq = engine.last_used();
        if any_victim.is_none_or(|(_, s)| seq < s) {
            any_victim = Some((*key, seq));
        }
        if engine.is_idle() && idle_victim.is_none_or(|(_, s)| seq < s) {
            idle_victim = Some((*key, seq));
        }
    }
    if let Some((key, _)) = idle_victim.or(any_victim) {
        slots.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 不依赖真实模型的哑引擎：缓存逻辑与 T 无关。
    struct Dummy;

    impl Dummy {
        fn build() -> Result<Self, String> {
            Ok(Dummy)
        }
    }

    fn cache_with(keys: &[u64]) -> EngineCache<Dummy> {
        let cache = EngineCache::new();
        for key in keys {
            cache.get_or_create(*key, Dummy::build).expect("建实例");
        }
        cache
    }

    fn keys_of(cache: &EngineCache<Dummy>) -> Vec<u64> {
        let mut keys: Vec<u64> = cache.slots.lock().keys().copied().collect();
        keys.sort_unstable();
        keys
    }

    #[test]
    fn holds_up_to_limit_then_evicts_least_recently_used() {
        let cache = cache_with(&[1, 2, 3, 4]);
        assert_eq!(keys_of(&cache), vec![1, 2, 3, 4]);

        // "最近使用"由真正取用（lock）刷新，而不是由 get_or_create 命中刷新。
        {
            let engine = cache.get_or_create(1, Dummy::build).expect("取 1");
            let _guard = engine.lock();
        }

        cache.get_or_create(5, Dummy::build).expect("取 5");

        let keys = keys_of(&cache);
        assert_eq!(keys.len(), MAX_CACHED_ENGINES, "不应超过上限：{keys:?}");
        assert!(!keys.contains(&2), "最久未用的 2 应被淘汰，实际 {keys:?}");
        assert!(keys.contains(&1), "刚用过的 1 不该被淘汰：{keys:?}");
        assert!(keys.contains(&5), "新键应在缓存里：{keys:?}");
    }

    #[test]
    fn eviction_skips_engine_currently_in_use() {
        let cache = cache_with(&[1, 2, 3, 4]);

        // 2 是最久未用的，但正被一次调用持有——淘汰必须绕开它。
        let holder = cache.get_or_create(2, Dummy::build).expect("取 2");
        let guard = holder.lock();
        cache.get_or_create(9, Dummy::build).expect("取 9");
        assert!(
            keys_of(&cache).contains(&2),
            "正在使用的实例不能被淘汰：{:?}",
            keys_of(&cache)
        );

        drop(guard);
    }

    #[test]
    fn release_unloads_engine_and_is_idempotent() {
        let cache = cache_with(&[7]);
        assert!(cache.release(7), "首次释放应命中");
        assert!(!cache.release(7), "重复释放应返回 false");
        assert!(keys_of(&cache).is_empty());
    }

    /// 淘汰过期实例后缓存仍要能正常取用（别把槽位锁或状态弄坏）。
    #[test]
    fn cache_still_usable_after_eviction() {
        let cache = cache_with(&[1, 2, 3, 4]);
        cache.get_or_create(5, Dummy::build).expect("触发淘汰");
        let engine = cache.get_or_create(1, Dummy::build).expect("淘汰后仍可取用");
        let _guard = engine.lock();
    }
}
