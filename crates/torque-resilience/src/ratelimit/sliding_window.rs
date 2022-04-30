//! Sliding window counter rate limiter with linear sub-window interpolation.
//!
//! Implements Cloudflare's sliding window algorithm:
//! `count = prev_window_count * (1.0 - time_into_current / window) + cur_window_count`.

use parking_lot::Mutex;
use torque_core::time::fast_now_millis;

/// Sliding window counter limiter.
pub struct SlidingWindowLimiter {
    window_duration_ms: u64,
    limit: u64,
    state: Mutex<WindowState>,
}

struct WindowState {
    current_window_start: u64,
    current_count: u64,
    previous_count: u64,
}

impl SlidingWindowLimiter {
    pub fn new(window_duration_ms: u64, limit: u64) -> Self {
        Self {
            window_duration_ms,
            limit,
            state: Mutex::new(WindowState {
                current_window_start: fast_now_millis(),
                current_count: 0,
                previous_count: 0,
            }),
        }
    }

    /// Attempt to acquire a token within the sliding window.
    pub fn try_acquire(&self, count: u64) -> Result<(), u64> {
        let now = fast_now_millis();
        let mut state = self.state.lock();

        // Advance window if elapsed
        let elapsed = now.saturating_sub(state.current_window_start);
        if elapsed >= self.window_duration_ms {
            let windows_passed = elapsed / self.window_duration_ms;
            if windows_passed == 1 {
                state.previous_count = state.current_count;
            } else {
                state.previous_count = 0;
            }
            state.current_count = 0;
            state.current_window_start = now - (elapsed % self.window_duration_ms);
        }

        let time_into_current = now.saturating_sub(state.current_window_start);
        let weight = 1.0 - (time_into_current as f64 / self.window_duration_ms as f64);
        let estimated_count = (state.previous_count as f64 * weight) + state.current_count as f64;

        if (estimated_count + count as f64) <= self.limit as f64 {
            state.current_count += count;
            Ok(())
        } else {
            let wait_ms = self
                .window_duration_ms
                .saturating_sub(time_into_current)
                .max(1);
            Err(wait_ms)
        }
    }
}
