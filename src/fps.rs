//! Frames per second over a one-second window, for the header.
use std::time::{Duration, Instant};

pub struct FpsMeter {
    start: Instant,
    count: u32,
    rate: u32,
}

impl FpsMeter {
    pub fn new(now: Instant) -> Self {
        Self {
            start: now,
            count: 0,
            rate: 0,
        }
    }

    /// Count `frames` new frames in the current window.
    pub fn add(&mut self, frames: u32) {
        self.count = self.count.saturating_add(frames);
    }

    /// Close the window once a second has passed. Returns true when the shown rate changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.start);
        if elapsed < Duration::from_secs(1) {
            return false;
        }
        let rate = (f64::from(self.count) / elapsed.as_secs_f64()).round() as u32;
        self.start = now;
        self.count = 0;
        let changed = rate != self.rate;
        self.rate = rate;
        changed
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }
}
