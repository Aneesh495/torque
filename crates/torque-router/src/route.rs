//! Route definitions, metadata, and match results.

use torque_proto::http1::Method;

use super::predicate::Predicate;
use super::trie::RouteParams;

/// Complete route rule definition with metadata and policies.
#[derive(Debug, Clone)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub path_pattern: String,
    pub methods: Vec<Method>,
    pub predicates: Vec<Predicate>,
    pub cluster: String,
    pub timeout_ms: Option<u64>,
    pub retry_count: Option<u32>,
    pub rate_limit_key: Option<String>,
    pub cache_ttl_secs: Option<u64>,
    pub plugins: Vec<String>,
}

impl Route {
    /// Construct a basic route mapped to an upstream cluster.
    pub fn new(
        name: impl Into<String>,
        path_pattern: impl Into<String>,
        cluster: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            id: name.clone(),
            name,
            path_pattern: path_pattern.into(),
            methods: Vec::new(),
            predicates: Vec::new(),
            cluster: cluster.into(),
            timeout_ms: None,
            retry_count: None,
            rate_limit_key: None,
            cache_ttl_secs: None,
            plugins: Vec::new(),
        }
    }

    /// Set allowed HTTP methods.
    pub fn with_methods(mut self, methods: Vec<Method>) -> Self {
        self.methods = methods;
        self
    }

    /// Add a matching predicate rule.
    pub fn with_predicate(mut self, predicate: Predicate) -> Self {
        self.predicates.push(predicate);
        self
    }

    /// Set upstream timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Result of matching a request against the route table.
#[derive(Debug, Clone)]
pub struct RouteMatch<'a> {
    pub route: &'a Route,
    pub params: RouteParams,
}
