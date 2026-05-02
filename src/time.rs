use chrono::{Duration, FixedOffset, Utc};

const CST_SECONDS: i32 = 8 * 3600;

fn cst_offset() -> FixedOffset {
    FixedOffset::east_opt(CST_SECONDS).expect("invalid timezone offset +08:00")
}

/// 当前时间 (CST, UTC+8) 的 RFC 3339 字符串
pub fn now_str() -> String {
    Utc::now().with_timezone(&cst_offset()).to_rfc3339()
}

/// 当前时间 + 指定偏移 (CST, UTC+8) 的 RFC 3339 字符串
pub fn now_str_add(duration: Duration) -> String {
    (Utc::now() + duration).with_timezone(&cst_offset()).to_rfc3339()
}

/// 当前 Unix 时间戳 (秒)，时区无关
pub fn now_epoch() -> u64 {
    Utc::now().timestamp() as u64
}
