//! Load balancing algorithm implementations.

pub mod ketama;
pub mod least_conn;
pub mod maglev;
pub mod peak_ewma;
pub mod swrr;

pub use ketama::KetamaRing;
pub use least_conn::LeastConnections;
pub use maglev::MaglevTable;
pub use peak_ewma::PeakEwma;
pub use swrr::SmoothWeightedRoundRobin;
