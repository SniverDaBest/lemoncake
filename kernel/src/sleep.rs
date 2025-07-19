use crate::interrupts::TICK_COUNT;
use core::sync::atomic::Ordering;

pub struct Sleep;

impl Sleep {
    pub fn ms(ms: u64) {
        let goal = TICK_COUNT.load(Ordering::Relaxed) + ms;
        loop {
            if TICK_COUNT.load(Ordering::Relaxed) >= goal {
                return;
            }
        }
    }

    pub fn sec(sec: u64) {
        let goal = TICK_COUNT.load(Ordering::Relaxed) + sec * 1000;
        loop {
            if TICK_COUNT.load(Ordering::Relaxed) >= goal {
                return;
            }
        }
    }
}
