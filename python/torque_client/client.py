"""Asynchronous client for interacting with the Torque administrative control plane."""

import json
import urllib.request
import urllib.error
from typing import Optional
from .models import ServerStats


class TorqueAdminClient:
    """Client for querying Torque proxy administrative and metrics endpoints."""

    def __init__(self, base_url: str = "http://127.0.0.1:9090", timeout: float = 5.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def is_alive(self) -> bool:
        """Query /healthz endpoint. Returns True if daemon is alive."""
        url = f"{self.base_url}/healthz"
        try:
            req = urllib.request.Request(url, method="GET")
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return resp.status == 200
        except Exception:
            return False

    def is_ready(self) -> bool:
        """Query /ready endpoint. Returns True if proxy is fully initialized."""
        url = f"{self.base_url}/ready"
        try:
            req = urllib.request.Request(url, method="GET")
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return resp.status == 200
        except Exception:
            return False

    def get_stats(self) -> ServerStats:
        """Fetch and parse live JSON metrics snapshot from /stats."""
        url = f"{self.base_url}/stats"
        req = urllib.request.Request(url, method="GET")
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            if resp.status != 200:
                raise RuntimeError(f"Admin server returned status {resp.status}")
            data = json.loads(resp.read().decode("utf-8"))
            return ServerStats.from_dict(data)

    def get_prometheus_metrics(self) -> str:
        """Retrieve raw Prometheus exposition text from /metrics."""
        url = f"{self.base_url}/metrics"
        req = urllib.request.Request(url, method="GET")
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            if resp.status != 200:
                raise RuntimeError(f"Admin server returned status {resp.status}")
            return resp.read().decode("utf-8")

    def trigger_hot_reload(self) -> bool:
        """Send POST /config/reload to trigger atomic zero-downtime configuration reload."""
        url = f"{self.base_url}/config/reload"
        req = urllib.request.Request(url, data=b"", method="POST")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return resp.status == 200
        except urllib.error.HTTPError as e:
            return False
