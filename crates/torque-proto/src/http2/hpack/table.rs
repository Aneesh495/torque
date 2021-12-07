//! HPACK Static and Dynamic header tables (RFC 7541 Section 2.3).

use std::collections::VecDeque;

/// A header field entry with name and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderField {
    pub name: String,
    pub value: String,
}

impl HeaderField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Estimated memory size in HPACK accounting: name.len() + value.len() + 32.
    pub fn size(&self) -> usize {
        self.name.len() + self.value.len() + 32
    }
}

/// Static table defined in RFC 7541 Appendix A (indices 1..=61).
pub static STATIC_TABLE: &[(&str, &str)] = &[
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// Dynamic header table with eviction based on maximum byte capacity.
#[derive(Debug, Clone)]
pub struct DynamicTable {
    entries: VecDeque<HeaderField>,
    size: usize,
    max_size: usize,
}

impl DynamicTable {
    /// Construct a dynamic table with given max byte size (default 4096 bytes).
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            size: 0,
            max_size,
        }
    }

    /// Insert a new header at index 1 of the dynamic table, evicting oldest if necessary.
    pub fn insert(&mut self, name: String, value: String) {
        let entry = HeaderField::new(name, value);
        let entry_size = entry.size();

        // Evict until new entry fits
        while self.size + entry_size > self.max_size && !self.entries.is_empty() {
            if let Some(evicted) = self.entries.pop_back() {
                self.size -= evicted.size();
            }
        }

        if entry_size <= self.max_size {
            self.size += entry_size;
            self.entries.push_front(entry);
        }
    }

    /// Retrieve an entry by 0-based dynamic index (index 0 corresponds to overall index 62).
    pub fn get(&self, index: usize) -> Option<&HeaderField> {
        self.entries.get(index)
    }

    /// Current memory size in bytes according to RFC 7541 accounting.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if dynamic table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Update max size and evict if current size exceeds new maximum.
    pub fn set_max_size(&mut self, new_max: usize) {
        self.max_size = new_max;
        while self.size > self.max_size && !self.entries.is_empty() {
            if let Some(evicted) = self.entries.pop_back() {
                self.size -= evicted.size();
            }
        }
    }
}

/// Unified table resolver combining static and dynamic tables.
pub struct HeaderTable {
    pub dynamic: DynamicTable,
}

impl HeaderTable {
    pub fn new(dynamic_max_size: usize) -> Self {
        Self {
            dynamic: DynamicTable::new(dynamic_max_size),
        }
    }

    /// Resolve an entry by 1-based index (1..=61 static, 62+ dynamic).
    pub fn get(&self, index: usize) -> Option<HeaderField> {
        if index == 0 {
            return None;
        }
        if index <= STATIC_TABLE.len() {
            let (name, val) = STATIC_TABLE[index - 1];
            Some(HeaderField::new(name, val))
        } else {
            let dynamic_idx = index - STATIC_TABLE.len() - 1;
            self.dynamic.get(dynamic_idx).cloned()
        }
    }

    /// Find match in static or dynamic table.
    /// Returns `(index, exact_value_match)`.
    pub fn find(&self, name: &str, value: &str) -> Option<(usize, bool)> {
        let mut name_match: Option<usize> = None;

        // Check static table first
        for (i, &(s_name, s_val)) in STATIC_TABLE.iter().enumerate() {
            if s_name == name {
                if s_val == value {
                    return Some((i + 1, true));
                }
                if name_match.is_none() {
                    name_match = Some(i + 1);
                }
            }
        }

        // Check dynamic table
        for (i, field) in self.dynamic.entries.iter().enumerate() {
            let overall_idx = STATIC_TABLE.len() + 1 + i;
            if field.name == name {
                if field.value == value {
                    return Some((overall_idx, true));
                }
                if name_match.is_none() {
                    name_match = Some(overall_idx);
                }
            }
        }

        name_match.map(|idx| (idx, false))
    }
}
