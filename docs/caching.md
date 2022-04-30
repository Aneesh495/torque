# Torque Caching & Adaptive Replacement Cache (ARC)

## 1. Megiddo-Modha Adaptive Replacement Cache (ARC)

Standard caching policies such as Least Recently Used (LRU) and Least Frequently Used (LFU) perform well under specific access patterns but fail under mixed workloads:
- LRU is vulnerable to sequential scans (a single large table dump evicts the entire cache).
- LFU is slow to adapt to changing working sets and accumulates stale high-frequency items.

To overcome these limitations, Torque implements the Megiddo-Modha Adaptive Replacement Cache (USENIX FAST 2003), a self-tuning, patent-free caching algorithm that dynamically balances recency and frequency.

```
+-------------------------------------------------------------+
|                     TOTAL CACHE CAPACITY c                  |
|                 [ T1 (Recency) ]  [ T2 (Frequency) ]        |
+-------------------------------------------------------------+
|                     GHOST HISTORY BUFFER c                  |
|                 [ B1 (Recent Ghosts) ] [ B2 (Freq Ghosts) ] |
+-------------------------------------------------------------+
```

---

## 2. Mathematical State Machine & Adaptation

ARC maintains four doubly-linked list structures tracking up to $2c$ metadata entries:

1. **$T_1$**: Cache entries accessed exactly once recently (Recency live set).
2. **$T_2$**: Cache entries accessed at least twice recently (Frequency live set).
3. **$B_1$**: Ghost directory of entries recently evicted from $T_1$ (stores keys and tags only, no payload).
4. **$B_2$**: Ghost directory of entries recently evicted from $T_2$.

The target size of $T_1$ is governed by an adaptive parameter $p \in [0, c]$.

### 2.1 Dynamic Adjustment Rules

- **Case 1: Hit in $T_1$ or $T_2$**:
  - Item is moved to the top (MRU position) of $T_2$. Parameter $p$ is unchanged.
- **Case 2: Hit in Ghost $B_1$**:
  - Indicates the cache should favor recency. Target $p$ expands:
    $$p \leftarrow \min\left(c, p + \max\left(1, \frac{|B_2|}{|B_1|}\right)\right)$$
  - Evicts a page from $T_1$ or $T_2$ via `replace()` and loads the item into $T_2$.
- **Case 3: Hit in Ghost $B_2$**:
  - Indicates the cache should favor frequency. Target $p$ contracts:
    $$p \leftarrow \max\left(0, p - \max\left(1, \frac{|B_1|}{|B_2|}\right)\right)$$
  - Evicts a page via `replace()` and loads the item into $T_2$.
- **Case 4: Cache Miss (Not in $T_1 \cup T_2 \cup B_1 \cup B_2$)**:
  - If $(|T_1| + |B_1|) = c$: evict LRU of $B_1$.
  - If $(|T_1| + |T_2| + |B_1| + |B_2|) = 2c$: evict LRU of $B_2$.
  - Call `replace()`, then insert new item into the MRU position of $T_1$.

### 2.2 Invariant Guarantees

Torque strictly preserves the core Megiddo-Modha capacity invariants:
1. $|T_1| + |T_2| \le c$ (live cached responses never exceed allocated memory).
2. $|T_1| + |B_1| \le c$
3. $|T_1| + |T_2| + |B_1| + |B_2| \le 2c$

---

## 3. RFC 7234 HTTP Freshness & Conditional Requests

The caching engine enforces RFC 7234 HTTP specifications:

- **Cache-Control Parsing**:
  - `no-store`: Never stores request or response.
  - `no-cache`: Forces origin revalidation before serving cached data.
  - `max-age=N`: Computes expiration timestamp $T_{\text{expire}} = T_{\text{now}} + N\text{ seconds}$.
- **Conditional Validation**:
  - If request provides `If-None-Match: "etag-value"` matching a cached response's `ETag`, Torque synthesizes an immediate `HTTP/1.1 304 Not Modified` response without contacting upstream.
  - Slashes outbound bandwidth and eliminates upstream server load.

---

## 4. Surrogate Tag Invalidation

To support instant content purging across distributed collections, Torque allows assigning arbitrary surrogate tags to cached responses (e.g. `user:101`, `catalog:electronics`):

- When an invalidation command arrives via admin API or filter (`cache_engine.invalidate_tag("user:101")`), all matching keys across $T_1, T_2, B_1, B_2$ are purged simultaneously in $O(1)$ amortized time.
