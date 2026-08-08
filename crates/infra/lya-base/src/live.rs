//! [`Live`]：可以整体换掉的共享快照。

use std::fmt;
use std::sync::{Arc, RwLock};

/// 一份读多写少的共享值，可以在运行时整体替换。
///
/// 存在的理由是「配置改了要重启才生效」这类 bug：装配时把配置**按值**拷进对象，
/// 之后配置文件改了，对象里那份拷贝没人动。换成 `Live` 之后，装配处留一个 handle，
/// 改配置时 [`set`](Live::set) 一下，持有方下次 [`get`](Live::get) 就拿到新的。
///
/// 语义是**整体替换**而不是逐字段改：拿到的 `Arc<T>` 是一份自洽的快照，用它做完
/// 一整件事都不会中途变样。调用方应当在一段工作开始时取一次，而不是每次用都取——
/// 否则同一轮里前半段用旧值、后半段用新值，比不生效更难查。
///
/// 锁中毒不会让它瘫痪：读写都从毒化的锁里把值捞出来。配置读取因为别处 panic 过
/// 就永久失灵，那是把一个局部故障放大成全局故障。
pub struct Live<T>(Arc<RwLock<Arc<T>>>);

impl<T> Live<T> {
    /// 用初值构造。
    pub fn new(value: T) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(value))))
    }

    /// 取当前快照。
    pub fn get(&self) -> Arc<T> {
        Arc::clone(&self.0.read().unwrap_or_else(|err| err.into_inner()))
    }

    /// 整体换成新值；此后的 [`get`](Live::get) 都拿到它。
    pub fn set(&self, value: T) {
        *self.0.write().unwrap_or_else(|err| err.into_inner()) = Arc::new(value);
    }
}

// 手写而不是 derive：derive 会顺手要求 `T: Clone`，可这里克隆的是 handle 本身，
// 跟 T 能不能克隆没有关系。Debug 同理。
impl<T> Clone for Live<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: fmt::Debug> fmt::Debug for Live<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Live").field(&*self.get()).finish()
    }
}

impl<T: Default> Default for Live<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// 让收 `impl Into<Live<T>>` 的构造函数继续接受一个裸值。
///
/// 只在装配处需要热替换；测试和示例传个定值就够了，不该被迫先包一层。
impl<T> From<T> for Live<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_is_visible_to_existing_handles() {
        let a = Live::new(1u32);
        let b = a.clone();
        b.set(2);
        // 换的是同一份，不是各自一份——否则装配处 set 完，持有方还看着旧值
        assert_eq!(*a.get(), 2);
    }

    #[test]
    fn snapshot_survives_later_set() {
        let live = Live::new(1u32);
        let held = live.get();
        live.set(2);
        // 取到手的快照不会被后来的 set 改写：一轮活干到一半不该换脚下的地
        assert_eq!(*held, 1);
        assert_eq!(*live.get(), 2);
    }

    #[test]
    fn poisoned_lock_still_serves() {
        let live = Live::new(1u32);
        // 必须在持有写锁的时候 panic 才真的毒化——set 完再 panic 是毒不到的
        let inner = Arc::clone(&live.0);
        let _ = std::thread::spawn(move || {
            let _guard = inner.write().unwrap();
            panic!("毒化这把锁");
        })
        .join();

        // 别处 panic 过就再也读不到配置，是把局部故障放大成全局故障
        assert_eq!(*live.get(), 1);
        live.set(3);
        assert_eq!(*live.get(), 3);
    }
}
