//! Radix tree (Patricia trie) for low-latency URL path routing.

use std::collections::HashMap;
use torque_core::error::{Result, TorqueError};

use super::segment::Segment;

/// Extracted path parameters from route matching.
pub type RouteParams = HashMap<String, String>;

/// Internal node of the routing Radix Trie.
pub struct TrieNode<T> {
    segment: Option<Segment>,
    exact_children: Vec<TrieNode<T>>,
    param_children: Vec<TrieNode<T>>,
    wildcard_child: Option<Box<TrieNode<T>>>,
    pub value: Option<T>,
}

impl<T> TrieNode<T> {
    /// Construct a new empty root or branch trie node.
    pub fn new(segment: Option<Segment>) -> Self {
        Self {
            segment,
            exact_children: Vec::new(),
            param_children: Vec::new(),
            wildcard_child: None,
            value: None,
        }
    }

    /// Insert a route path and associated value into the trie.
    pub fn insert(&mut self, path: &str, val: T) -> Result<()> {
        let clean_path = path.trim_matches('/');
        let tokens: Vec<&str> = if clean_path.is_empty() {
            Vec::new()
        } else {
            clean_path.split('/').collect()
        };

        let mut current = self;
        for (i, &token) in tokens.iter().enumerate() {
            let seg = Segment::parse(token)?;
            match seg {
                Segment::Exact(ref name) => {
                    let mut found_idx = None;
                    for (c_idx, child) in current.exact_children.iter().enumerate() {
                        if let Some(Segment::Exact(ref cname)) = child.segment {
                            if cname == name {
                                found_idx = Some(c_idx);
                                break;
                            }
                        }
                    }
                    let idx = match found_idx {
                        Some(idx) => idx,
                        None => {
                            current.exact_children.push(TrieNode::new(Some(seg)));
                            current.exact_children.len() - 1
                        }
                    };
                    current = &mut current.exact_children[idx];
                }
                Segment::Param {
                    ref name,
                    ref regex,
                } => {
                    let mut found_idx = None;
                    for (c_idx, child) in current.param_children.iter().enumerate() {
                        if let Some(Segment::Param {
                            name: ref cname,
                            regex: ref cregex,
                        }) = child.segment
                        {
                            let regex_eq = match (regex, cregex) {
                                (Some(r1), Some(r2)) => r1.as_str() == r2.as_str(),
                                (None, None) => true,
                                _ => false,
                            };
                            if cname == name && regex_eq {
                                found_idx = Some(c_idx);
                                break;
                            }
                        }
                    }
                    let idx = match found_idx {
                        Some(idx) => idx,
                        None => {
                            // Place constrained regex children before unconstrained ones
                            let has_regex = regex.is_some();
                            let node = TrieNode::new(Some(seg));
                            if has_regex {
                                current.param_children.insert(0, node);
                                0
                            } else {
                                current.param_children.push(node);
                                current.param_children.len() - 1
                            }
                        }
                    };
                    current = &mut current.param_children[idx];
                }
                Segment::Wildcard(_) => {
                    if i != tokens.len() - 1 {
                        return Err(TorqueError::config(
                            "router",
                            "Wildcard must be the terminal segment in a route path",
                        ));
                    }
                    if current.wildcard_child.is_none() {
                        current.wildcard_child = Some(Box::new(TrieNode::new(Some(seg))));
                    }
                    current = current.wildcard_child.as_mut().unwrap();
                }
            }
        }

        current.value = Some(val);
        Ok(())
    }

    /// Look up a path in the trie, populating captured parameters.
    pub fn lookup<'a>(&'a self, path: &str, params: &mut RouteParams) -> Option<&'a T> {
        let clean_path = path.trim_matches('/');
        let tokens: Vec<&str> = if clean_path.is_empty() {
            Vec::new()
        } else {
            clean_path.split('/').collect()
        };

        self.lookup_recursive(&tokens, 0, params)
    }

    fn lookup_recursive<'a>(
        &'a self,
        tokens: &[&str],
        depth: usize,
        params: &mut RouteParams,
    ) -> Option<&'a T> {
        if depth == tokens.len() {
            return self.value.as_ref();
        }

        let token = tokens[depth];

        // 1. Prioritize exact matching child
        for child in &self.exact_children {
            if let Some(Segment::Exact(ref name)) = child.segment {
                if name == token {
                    if let Some(val) = child.lookup_recursive(tokens, depth + 1, params) {
                        return Some(val);
                    }
                }
            }
        }

        // 2. Try parameterized children (constrained regex first, then unconstrained)
        for param_node in &self.param_children {
            if let Some(Segment::Param {
                ref name,
                ref regex,
            }) = param_node.segment
            {
                let matches = match regex {
                    Some(re) => re.is_match(token),
                    None => true,
                };
                if matches {
                    params.insert(name.clone(), token.to_string());
                    if let Some(val) = param_node.lookup_recursive(tokens, depth + 1, params) {
                        return Some(val);
                    }
                    params.remove(name);
                }
            }
        }

        // 3. Try wildcard child
        if let Some(ref wild_node) = self.wildcard_child {
            if let Some(Segment::Wildcard(ref name)) = wild_node.segment {
                let rest = tokens[depth..].join("/");
                params.insert(name.clone(), rest);
                return wild_node.value.as_ref();
            }
        }

        None
    }
}
