//! Route predicates for multi-attribute request matching.

use regex::Regex;
use torque_proto::http1::Request;

/// Predicate matching rule.
#[derive(Debug, Clone)]
pub enum Predicate {
    /// Request header matches value exactly or by regex.
    Header {
        name: String,
        exact: Option<String>,
        regex: Option<Regex>,
    },
    /// Query parameter matches value or regex.
    QueryParam {
        name: String,
        exact: Option<String>,
        regex: Option<Regex>,
    },
    /// Host header or SNI matches pattern (exact or `*.domain`).
    Host {
        pattern: String,
    },
    /// Method matches specified list.
    MethodIn {
        methods: Vec<String>,
    },
}

impl Predicate {
    /// Construct an exact header match predicate.
    pub fn header_exact(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Header {
            name: name.into(),
            exact: Some(value.into()),
            regex: None,
        }
    }

    /// Construct a regex header match predicate.
    pub fn header_regex(name: impl Into<String>, pattern: &str) -> Result<Self, regex::Error> {
        let re = Regex::new(pattern)?;
        Ok(Self::Header {
            name: name.into(),
            exact: None,
            regex: Some(re),
        })
    }

    /// Construct a host match predicate.
    pub fn host(pattern: impl Into<String>) -> Self {
        Self::Host {
            pattern: pattern.into(),
        }
    }

    /// Evaluate if an incoming request satisfies this predicate.
    pub fn evaluate(&self, req: &Request) -> bool {
        match self {
            Predicate::Header { name, exact, regex } => {
                match req.headers.get(name) {
                    Some(val) => {
                        if let Some(ref exp) = exact {
                            if exp != val {
                                return false;
                            }
                        }
                        if let Some(ref re) = regex {
                            if !re.is_match(val) {
                                return false;
                            }
                        }
                        true
                    }
                    None => false,
                }
            }
            Predicate::QueryParam { name, exact, regex } => {
                // Parse query string from URI
                if let Some(q_idx) = req.uri.find('?') {
                    let query = &req.uri[q_idx + 1..];
                    for pair in query.split('&') {
                        let mut kv = pair.splitn(2, '=');
                        let k = kv.next().unwrap_or("");
                        let v = kv.next().unwrap_or("");
                        if k == name {
                            if let Some(ref exp) = exact {
                                if exp != v {
                                    return false;
                                }
                            }
                            if let Some(ref re) = regex {
                                if !re.is_match(v) {
                                    return false;
                                }
                            }
                            return true;
                        }
                    }
                }
                false
            }
            Predicate::Host { pattern } => {
                let host = req.headers.get("host").unwrap_or("");
                // Strip port if present
                let host_domain = host.split(':').next().unwrap_or("");
                if pattern.starts_with("*.") {
                    let suffix = &pattern[1..]; // ".example.com"
                    host_domain.ends_with(suffix)
                } else {
                    pattern == host_domain
                }
            }
            Predicate::MethodIn { methods } => {
                let m = req.method.as_str();
                methods.iter().any(|allowed| allowed.eq_ignore_ascii_case(m))
            }
        }
    }
}
