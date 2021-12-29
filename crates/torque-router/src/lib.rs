//! # Torque Router
//!
//! Radix Patricia trie path matching, parameterized segment extraction,
//! multi-attribute predicate evaluation, and atomic lock-free route swapping.

pub mod predicate;
pub mod route;
pub mod segment;
pub mod table;
pub mod trie;

pub use predicate::Predicate;
pub use route::{Route, RouteMatch};
pub use segment::Segment;
pub use table::{AtomicRouteTable, RouteTable};
pub use trie::{RouteParams, TrieNode};

/// Router prelude.
pub mod prelude {
    pub use crate::predicate::Predicate;
    pub use crate::route::{Route, RouteMatch};
    pub use crate::table::{AtomicRouteTable, RouteTable};
    pub use crate::trie::RouteParams;
}
