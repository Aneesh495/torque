//! HTTP/2 connection and stream flow control window management (RFC 7540 Section 5.2).

use torque_core::error::{Result, TorqueError};

/// Standard default initial flow control window size (65,535 octets).
pub const DEFAULT_INITIAL_WINDOW_SIZE: i32 = 65_535;

/// Maximum allowable window size (2^31 - 1 octets).
pub const MAX_WINDOW_SIZE: i32 = 0x7FFF_FFFF;

/// Flow control window tracker.
#[derive(Debug, Clone)]
pub struct FlowControl {
    window_size: i32,
    initial_window: i32,
}

impl FlowControl {
    /// Construct flow control tracker with initial window size.
    pub fn new(initial_window: i32) -> Self {
        Self {
            window_size: initial_window,
            initial_window,
        }
    }

    /// Current available window size in bytes. Can be negative if peer reduced initial window.
    pub fn available(&self) -> i32 {
        self.window_size
    }

    /// Consume credit for transmitted or received data payload.
    pub fn consume(&mut self, amount: usize) -> Result<()> {
        let amount = amount as i32;
        self.window_size -= amount;
        Ok(())
    }

    /// Process incoming WINDOW_UPDATE frame.
    pub fn update(&mut self, increment: u32) -> Result<()> {
        if increment == 0 {
            return Err(TorqueError::protocol("WINDOW_UPDATE increment of 0"));
        }
        let inc = increment as i64;
        let new_size = (self.window_size as i64) + inc;
        if new_size > (MAX_WINDOW_SIZE as i64) {
            return Err(TorqueError::protocol(
                "Flow control window exceeded maximum",
            ));
        }
        self.window_size = new_size as i32;
        Ok(())
    }

    /// Check if window has depleted below threshold (e.g., 50% of initial) and requires WINDOW_UPDATE.
    pub fn needs_update(&self) -> Option<u32> {
        let threshold = self.initial_window / 2;
        if self.window_size < threshold {
            let delta = self.initial_window - self.window_size;
            if delta > 0 {
                return Some(delta as u32);
            }
        }
        None
    }
}

impl Default for FlowControl {
    fn default() -> Self {
        Self::new(DEFAULT_INITIAL_WINDOW_SIZE)
    }
}
