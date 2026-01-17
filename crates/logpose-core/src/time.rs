pub trait Clock {
    fn now_millis(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        now.as_millis() as u64
    }
}
pub fn now() -> u64 {
    SystemClock.now_millis()
}
