//! URL path segment parsing and validation.

use regex::Regex;
use torque_core::error::{Result, TorqueError};

/// Parsed segment type of a route path pattern.
#[derive(Debug, Clone)]
pub enum Segment {
    /// Exact literal match (e.g., "users")
    Exact(String),
    /// Parameter capture with optional regex constraint (e.g., ":id" or ":id<\\d+>")
    Param { name: String, regex: Option<Regex> },
    /// Catch-all greedy wildcard (e.g., "*filepath")
    Wildcard(String),
}

impl Segment {
    /// Parse a single segment string token into a Segment enum.
    pub fn parse(token: &str) -> Result<Self> {
        if let Some(rest) = token.strip_prefix('*') {
            let name = if !rest.is_empty() {
                rest.to_string()
            } else {
                "wildcard".to_string()
            };
            Ok(Segment::Wildcard(name))
        } else if let Some(remainder) = token.strip_prefix(':') {
            if let Some(open_angle) = remainder.find('<') {
                if !remainder.ends_with('>') {
                    return Err(TorqueError::config(
                        "router",
                        format!("Unclosed regex constraint in parameter segment: {token}"),
                    ));
                }
                let name = remainder[..open_angle].to_string();
                let pattern = &remainder[open_angle + 1..remainder.len() - 1];
                let re = Regex::new(pattern).map_err(|e| {
                    TorqueError::config("router", format!("Invalid regex '{pattern}': {e}"))
                })?;
                Ok(Segment::Param {
                    name,
                    regex: Some(re),
                })
            } else {
                Ok(Segment::Param {
                    name: remainder.to_string(),
                    regex: None,
                })
            }
        } else {
            Ok(Segment::Exact(token.to_string()))
        }
    }

    /// Match a concrete path segment against this pattern segment.
    pub fn matches(&self, candidate: &str) -> bool {
        match self {
            Segment::Exact(expected) => expected == candidate,
            Segment::Param { regex, .. } => match regex {
                Some(re) => re.is_match(candidate),
                None => true,
            },
            Segment::Wildcard(_) => true,
        }
    }
}
