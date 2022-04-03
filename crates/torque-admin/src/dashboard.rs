//! Embedded Single-Page Application (SPA) dashboard served by the admin daemon.

/// Static HTML/CSS/JS dashboard string served at `/dashboard`.
pub static DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Torque Control Plane</title>
  <style>
    :root {
      --bg: #0d1117;
      --card-bg: #161b22;
      --border: #30363d;
      --text: #c9d1d9;
      --text-dim: #8b949e;
      --accent: #58a6ff;
      --green: #3fb950;
      --red: #f85149;
      --font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
      line-height: 1.5;
      padding: 24px;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding-bottom: 20px;
      border-bottom: 1px solid var(--border);
      margin-bottom: 24px;
    }
    h1 { font-size: 24px; font-weight: 600; color: #fff; display: flex; align-items: center; gap: 10px; }
    .badge {
      background: rgba(88, 166, 255, 0.15);
      color: var(--accent);
      font-size: 12px;
      padding: 2px 8px;
      border-radius: 12px;
      border: 1px solid rgba(88, 166, 255, 0.3);
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }
    .card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 16px;
    }
    .card-title {
      font-size: 13px;
      color: var(--text-dim);
      text-transform: uppercase;
      letter-spacing: 0.5px;
      margin-bottom: 8px;
    }
    .card-value {
      font-size: 28px;
      font-weight: 700;
      color: #fff;
      font-family: var(--font-mono);
    }
    .card-sub { font-size: 12px; color: var(--text-dim); margin-top: 4px; }
    .btn {
      background: #238636;
      color: #fff;
      border: 1px solid rgba(240, 246, 252, 0.1);
      padding: 8px 16px;
      border-radius: 6px;
      cursor: pointer;
      font-weight: 600;
      font-size: 14px;
    }
    .btn:hover { background: #2ea043; }
    pre {
      background: #090d13;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 16px;
      overflow-x: auto;
      font-family: var(--font-mono);
      font-size: 13px;
      color: #7ee787;
    }
  </style>
</head>
<body>
  <header>
    <div>
      <h1>Torque Edge Control Plane <span class="badge">ACTIVE</span></h1>
      <div style="font-size: 13px; color: var(--text-dim); margin-top: 4px;">Programmable Layer-7 Reverse Proxy & Traffic Engine</div>
    </div>
    <div>
      <button class="btn" onclick="triggerReload()">Trigger Hot-Reload</button>
    </div>
  </header>

  <div class="grid">
    <div class="card">
      <div class="card-title">Total Requests</div>
      <div class="card-value" id="val-total">0</div>
      <div class="card-sub" id="val-status-breakdown">2xx: 0 | 4xx: 0 | 5xx: 0</div>
    </div>
    <div class="card">
      <div class="card-title">Active Connections</div>
      <div class="card-value" id="val-conns">0</div>
      <div class="card-sub">TCP worker socket pool</div>
    </div>
    <div class="card">
      <div class="card-title">Latency p50 / p99</div>
      <div class="card-value" id="val-latency">0.0ms</div>
      <div class="card-sub" id="val-latency-sub">HDR logarithmic histogram</div>
    </div>
    <div class="card">
      <div class="card-title">Traffic Transmitted</div>
      <div class="card-value" id="val-bytes">0 KB</div>
      <div class="card-sub" id="val-bytes-sub">Zero-copy vectored I/O</div>
    </div>
  </div>

  <div class="card">
    <div class="card-title">Live Metrics Telemetry (/stats)</div>
    <pre id="raw-stats">Loading cluster telemetry...</pre>
  </div>

  <script>
    async function fetchStats() {
      try {
        const res = await fetch('/stats');
        if (res.ok) {
          const data = await res.json();
          document.getElementById('val-total').innerText = Number(data.requests.total).toLocaleString();
          document.getElementById('val-status-breakdown').innerText = `2xx: ${data.requests['2xx']} | 4xx: ${data.requests['4xx']} | 5xx: ${data.requests['5xx']}`;
          document.getElementById('val-conns').innerText = data.connections;
          document.getElementById('val-latency').innerText = `${data.latency.p50.toFixed(2)}ms / ${data.latency.p99.toFixed(2)}ms`;
          document.getElementById('val-latency-sub').innerText = `Mean: ${data.latency.mean.toFixed(2)}ms | p90: ${data.latency.p90.toFixed(2)}ms`;
          const kb = (data.bytes.sent / 1024).toFixed(1);
          document.getElementById('val-bytes').innerText = `${kb} KB`;
          document.getElementById('raw-stats').innerText = JSON.stringify(data, null, 2);
        }
      } catch (e) {
        console.error('Stats error:', e);
      }
    }

    async function triggerReload() {
      try {
        const res = await fetch('/config/reload', { method: 'POST' });
        if (res.ok) {
          alert('Configuration reload signal dispatched successfully!');
          fetchStats();
        } else {
          alert('Reload failed: ' + res.statusText);
        }
      } catch (e) {
        alert('Reload error: ' + e);
      }
    }

    fetchStats();
    setInterval(fetchStats, 1000);
  </script>
</body>
</html>
"#;
