# Torque Resilience & Fault Tolerance Engine

## 1. Multi-Tier Resilience Architecture

Microservice architectures depend on robust failure containment. Torque provides an end-to-end resilience pipeline guarding against traffic spikes, slow upstream queries, network partitions, and cascading collapse:

```
[Incoming Request]
       |
       v
+-----------------------------+
| 1. Token / Sliding Limiter  | ---> [429 Too Many Requests]
+-----------------------------+
       | Passed
       v
+-----------------------------+
| 2. 4-State Circuit Breaker  | ---> [503 Circuit Breaker Open]
+-----------------------------+
       | Allowed
       v
+-----------------------------+
| 3. Concurrency Limiter      | ---> [Adaptive Queue Delay]
+-----------------------------+
       | Dispatched
       v
+-----------------------------+
| 4. Jittered Retry Budget    | ---> [Backoff Loop (Full Jitter)]
+-----------------------------+
```

---

## 2. 4-State Circuit Breaker

The Torque `CircuitBreaker` manages upstream reliability through four operating states:

1. **Closed**: Normal operational state. All calls pass to upstream.
   - Failures and slow calls are recorded in a sliding time window.
   - If minimum request volume is met and the failure rate $\ge$ threshold, trips to `Open`.
2. **Open**: Fault isolation state.
   - All requests are rejected immediately with HTTP 503 and header `X-Torque-Circuit: OPEN`.
   - Prevents overwhelming degraded services and frees downstream caller resources.
   - Stays Open until `recovery_timeout_ms` elapses, then transitions to `HalfOpen`.
3. **HalfOpen**: Controlled trial recovery state.
   - Permits a strictly bounded number of trial probe requests (`half_open_probes`).
   - If all probes succeed, resets circuit to `Closed` and clears error history.
   - If any probe fails, immediately trips back to `Open`.
4. **ForcedOpen**: Administrative maintenance override.
   - Hard isolation commanded via admin control plane.

### 2.1 Sliding Window Metric Bucketing

Failure tracking uses a high-resolution sliding window composed of sixty 100ms sub-buckets covering a rolling 6-second window. Each sub-bucket tracks total calls, failure calls, and slow calls using atomic increments. As time advances, expired buckets are recycled without dynamic allocation.

---

## 3. Rate Limiting Algorithms

Torque provides three complementary rate limiting algorithms tailored to distinct traffic profiles:

### 3.1 High-Precision Token Bucket

- Tracks tokens using fractional atomic integers.
- Tokens regenerate continuously based on monotonic nanosecond elapsed time:
  $$\Delta \text{tokens} = \frac{\Delta t_{\text{nanos}} \times R}{10^9}$$
- Accommodates configured burst capacities while enforcing strict steady-state limits.

### 3.2 Leaky Bucket

- Requests enter a bounded FIFO buffer and leak out at a constant rate $R$.
- Smooths bursty inbound traffic into uniform downstream flow, eliminating spikes.

### 3.3 Cloudflare Sub-Window Sliding Limiter

- Blends the transaction count of the previous time window $C_{\text{prev}}$ with the current counter $C_{\text{curr}}$ based on elapsed fraction $f \in [0, 1]$:
  $$\text{EstimatedCount} = C_{\text{prev}} \times (1 - f) + C_{\text{curr}}$$
- Prevents boundary-crossing burst attacks (where clients send double their limit across window boundaries) with minimal memory overhead ($O(1)$ per key).

---

## 4. Jittered Exponential Backoff & Retry Budgeting

Retrying failed requests naively causes synchronized retry storms that keep failing upstreams permanently down. Torque breaks resonance through AWS-style jittered backoffs:

- **Full Jitter**:
  $$t = \text{Uniform}(0, \min(\text{Cap}, \text{Base} \cdot 2^{\text{attempt}}))$$
- **Decorrelated Jitter**:
  $$t_{i+1} = \min(\text{Cap}, \text{Uniform}(\text{Base}, t_i \cdot 3))$$

### 4.1 Retry Budget Token Bucket

To prevent retry amplification in deep dependency graphs, Torque assigns an atomic `RetryBudget`:
- Successful responses replenish the retry budget (e.g. 10% credit).
- Failed requests consume budget tokens.
- When budget is exhausted, further retries are suppressed and the original error is returned immediately.
