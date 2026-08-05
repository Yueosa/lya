//! 测试用的临时库。
//!
//! 建库这件事在测试里出现了十几处，每处都要 tempdir + open + migrate。放在这里
//! 是为了让「怎么建一个完整的库」只有一个答案——schema 加表时不用再去挨个改测试。

use std::sync::Arc;

use tempfile::TempDir;

use crate::Db;

/// 在临时目录里开一个建好全部表的库。
///
/// [`TempDir`] 必须留在作用域内：它一 drop 库文件就跟着没了。
pub fn open_test_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("建临时目录");
    let db = Db::open(dir.path().join("lya.db")).expect("打开临时库");
    db.migrate().expect("建表");
    (dir, Arc::new(db))
}
