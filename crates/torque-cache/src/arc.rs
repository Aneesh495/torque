//! Adaptive Replacement Cache (ARC) memory storage engine.
//!
//! Based on the USENIX FAST 2003 paper by Nimrod Megiddo and Dharmendra S. Modha.
//! Dynamically tunes target size `p` between recency (LRU, T1) and frequency (LFU, T2)
//! using ghost history tracking (B1, B2).

use std::collections::{HashMap, VecDeque};

/// Cached entry value stored in memory.
#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    pub key: String,
    pub value: V,
    pub expires_at_millis: u64,
    pub tags: Vec<String>,
}

/// Adaptive Replacement Cache (ARC) state machine.
pub struct AdaptiveReplacementCache<V> {
    capacity: usize,
    p: usize, // Target size for T1 list
    t1: VecDeque<String>,
    t2: VecDeque<String>,
    b1: VecDeque<String>,
    b2: VecDeque<String>,
    data: HashMap<String, CacheEntry<V>>,
}

impl<V: Clone> AdaptiveReplacementCache<V> {
    /// Construct a new ARC cache with fixed maximum item capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero");
        Self {
            capacity,
            p: 0,
            t1: VecDeque::new(),
            t2: VecDeque::new(),
            b1: VecDeque::new(),
            b2: VecDeque::new(),
            data: HashMap::new(),
        }
    }

    /// Retrieve an item from the cache, updating recency/frequency position.
    pub fn get(&mut self, key: &str) -> Option<CacheEntry<V>> {
        if !self.data.contains_key(key) {
            return None;
        }

        // Case 1: Hit in T1 or T2 -> Move to MRU position of T2
        if let Some(pos) = self.t1.iter().position(|k| k == key) {
            let k = self.t1.remove(pos).unwrap();
            self.t2.push_front(k);
        } else if let Some(pos) = self.t2.iter().position(|k| k == key) {
            let k = self.t2.remove(pos).unwrap();
            self.t2.push_front(k);
        }

        self.data.get(key).cloned()
    }

    /// Insert or update an item in the cache.
    pub fn put(&mut self, key: String, value: V, expires_at_millis: u64, tags: Vec<String>) {
        let entry = CacheEntry {
            key: key.clone(),
            value,
            expires_at_millis,
            tags,
        };

        // Case 1: Key already in T1 or T2
        if self.data.contains_key(&key) {
            self.data.insert(key.clone(), entry);
            if let Some(pos) = self.t1.iter().position(|k| k == &key) {
                let k = self.t1.remove(pos).unwrap();
                self.t2.push_front(k);
            } else if let Some(pos) = self.t2.iter().position(|k| k == &key) {
                let k = self.t2.remove(pos).unwrap();
                self.t2.push_front(k);
            }
            return;
        }

        // Case 2: Key in B1 (ghost list of recent items) -> Increase p (bias toward recency)
        if let Some(pos) = self.b1.iter().position(|k| k == &key) {
            self.b1.remove(pos);
            let b1_len = self.b1.len().max(1);
            let b2_len = self.b2.len().max(1);
            let delta = if b1_len >= b2_len { 1 } else { b2_len / b1_len };
            self.p = (self.p + delta).min(self.capacity);
            self.replace(false);
            self.t2.push_front(key.clone());
            self.data.insert(key, entry);
            return;
        }

        // Case 3: Key in B2 (ghost list of frequent items) -> Decrease p (bias toward frequency)
        if let Some(pos) = self.b2.iter().position(|k| k == &key) {
            self.b2.remove(pos);
            let b1_len = self.b1.len().max(1);
            let b2_len = self.b2.len().max(1);
            let delta = if b2_len >= b1_len { 1 } else { b1_len / b2_len };
            self.p = self.p.saturating_sub(delta);
            self.replace(true);
            self.t2.push_front(key.clone());
            self.data.insert(key, entry);
            return;
        }

        // Case 4: Cache miss in all lists
        let live_count = self.t1.len() + self.t2.len();
        if live_count >= self.capacity {
            // Cache is full; must evict an active entry before adding new one
            let l1 = self.t1.len() + self.b1.len();
            if l1 == self.capacity {
                if self.t1.len() < self.capacity {
                    self.b1.pop_back();
                    self.replace(false);
                } else if let Some(k) = self.t1.pop_back() {
                    self.data.remove(&k);
                }
            } else {
                let total = self.t1.len() + self.t2.len() + self.b1.len() + self.b2.len();
                if total >= (2 * self.capacity) {
                    if !self.b2.is_empty() {
                        self.b2.pop_back();
                    } else if !self.b1.is_empty() {
                        self.b1.pop_back();
                    }
                }
                self.replace(false);
            }
        } else {
            // Cache is not full yet; trim ghost lists if total history exceeds 2c
            let total = self.t1.len() + self.t2.len() + self.b1.len() + self.b2.len();
            if total >= (2 * self.capacity) {
                if !self.b2.is_empty() {
                    self.b2.pop_back();
                } else if !self.b1.is_empty() {
                    self.b1.pop_back();
                }
            }
        }

        self.t1.push_front(key.clone());
        self.data.insert(key, entry);
    }

    /// Evict an item from T1 or T2 into corresponding ghost list B1 or B2.
    fn replace(&mut self, hit_in_b2: bool) {
        let t1_len = self.t1.len();
        if t1_len > 0 && (t1_len > self.p || (hit_in_b2 && t1_len == self.p)) {
            if let Some(k) = self.t1.pop_back() {
                self.b1.push_front(k.clone());
                self.data.remove(&k);
            }
        } else if let Some(k) = self.t2.pop_back() {
            self.b2.push_front(k.clone());
            self.data.remove(&k);
        } else if let Some(k) = self.t1.pop_back() {
            self.b1.push_front(k.clone());
            self.data.remove(&k);
        }
    }

    /// Remove a specific key from the cache.
    pub fn remove(&mut self, key: &str) -> Option<CacheEntry<V>> {
        self.t1.retain(|k| k != key);
        self.t2.retain(|k| k != key);
        self.b1.retain(|k| k != key);
        self.b2.retain(|k| k != key);
        self.data.remove(key)
    }

    /// Invalidate all entries matching a specific cache tag.
    pub fn invalidate_tag(&mut self, tag: &str) -> usize {
        let mut to_remove = Vec::new();
        for (k, entry) in &self.data {
            if entry.tags.iter().any(|t| t == tag) {
                to_remove.push(k.clone());
            }
        }
        for k in &to_remove {
            self.remove(k);
        }
        to_remove.len()
    }

    /// Number of active items currently stored with data in cache.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Maximum capacity of the cache.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current adaptive parameter p.
    pub fn target_p(&self) -> usize {
        self.p
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
