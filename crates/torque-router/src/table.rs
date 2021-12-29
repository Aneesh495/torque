//! Compiled route table and lock-free atomic snapshot swap.

use arc_swap::ArcSwap;
use std::sync::Arc;
use torque_core::error::Result;
use torque_proto::http1::Request;

use super::route::{Route, RouteMatch};
use super::trie::{RouteParams, TrieNode};

/// Compiled immutable routing table snapshot.
pub struct RouteTable {
    routes: Vec<Route>,
    trie: TrieNode<Vec<usize>>,
}

impl RouteTable {
    /// Compile a list of routes into a search trie.
    pub fn compile(routes: Vec<Route>) -> Result<Self> {
        let mut trie: TrieNode<Vec<usize>> = TrieNode::new(None);

        for (idx, route) in routes.iter().enumerate() {
            // Strip query string if present in route path
            let path = route.path_pattern.split('?').next().unwrap();

            // Insert route index into trie node's route list
            let mut existing: Vec<usize> = Vec::new();
            let mut params = RouteParams::new();
            if let Some(list) = trie.lookup(path, &mut params) {
                existing = list.clone();
            }
            existing.push(idx);
            trie.insert(path, existing)?;
        }

        Ok(Self { routes, trie })
    }

    /// Match an incoming HTTP request against the routing table.
    pub fn match_request<'a>(&'a self, req: &Request) -> Option<RouteMatch<'a>> {
        // Strip query string from request URI
        let path = req.uri.split('?').next().unwrap_or(&req.uri);
        let mut params = RouteParams::new();

        if let Some(candidate_indices) = self.trie.lookup(path, &mut params) {
            for &idx in candidate_indices {
                let route = &self.routes[idx];

                // Method match check
                if !route.methods.is_empty() && !route.methods.contains(&req.method) {
                    continue;
                }

                // Predicate match checks
                let all_predicates_pass = route.predicates.iter().all(|pred| pred.evaluate(req));
                if !all_predicates_pass {
                    continue;
                }

                return Some(RouteMatch {
                    route,
                    params: params.clone(),
                });
            }
        }

        None
    }

    /// Retrieve all configured routes in this table.
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    /// Total number of configured routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if route table has zero routes.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

/// Lock-free atomic holder for active RouteTable allowing zero-downtime hot swapping.
pub struct AtomicRouteTable {
    inner: ArcSwap<RouteTable>,
}

impl AtomicRouteTable {
    /// Construct atomic route table with initial compiled table.
    pub fn new(initial: RouteTable) -> Self {
        Self {
            inner: ArcSwap::from_pointee(initial),
        }
    }

    /// Load current active route table snapshot.
    pub fn load(&self) -> Arc<RouteTable> {
        self.inner.load_full()
    }

    /// Atomically swap active route table with a newly compiled snapshot.
    pub fn swap(&self, new_table: RouteTable) {
        self.inner.store(Arc::new(new_table));
    }
}
