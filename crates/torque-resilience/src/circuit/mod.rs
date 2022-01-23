//! Circuit breaker subsystem.

pub mod breaker;
pub mod state;
pub mod window;

pub use breaker::{CircuitBreaker, CircuitBreakerConfig};
pub use state::CircuitState;
pub use window::{SlidingWindow, WindowSnapshot};
