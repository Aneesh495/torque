"""Strongly-typed data models for Torque telemetry, health, and server metrics."""

from dataclasses import dataclass
from typing import Dict, Any


@dataclass
class RequestBreakdown:
    total: int
    status_2xx: int
    status_3xx: int
    status_4xx: int
    status_5xx: int

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "RequestBreakdown":
        return cls(
            total=int(data.get("total", 0)),
            status_2xx=int(data.get("2xx", 0)),
            status_3xx=int(data.get("3xx", 0)),
            status_4xx=int(data.get("4xx", 0)),
            status_5xx=int(data.get("5xx", 0)),
        )


@dataclass
class BandwidthMetrics:
    sent_bytes: int
    received_bytes: int

    @property
    def sent_kb(self) -> float:
        return self.sent_bytes / 1024.0

    @property
    def received_kb(self) -> float:
        return self.received_bytes / 1024.0

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "BandwidthMetrics":
        return cls(
            sent_bytes=int(data.get("sent", 0)),
            received_bytes=int(data.get("received", 0)),
        )


@dataclass
class LatencyMetrics:
    mean_ms: float
    p50_ms: float
    p90_ms: float
    p99_ms: float

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "LatencyMetrics":
        return cls(
            mean_ms=float(data.get("mean", 0.0)),
            p50_ms=float(data.get("p50", 0.0)),
            p90_ms=float(data.get("p90", 0.0)),
            p99_ms=float(data.get("p99", 0.0)),
        )


@dataclass
class ServerStats:
    requests: RequestBreakdown
    connections: int
    bytes: BandwidthMetrics
    latency: LatencyMetrics

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ServerStats":
        return cls(
            requests=RequestBreakdown.from_dict(data.get("requests", {})),
            connections=int(data.get("connections", 0)),
            bytes=BandwidthMetrics.from_dict(data.get("bytes", {})),
            latency=LatencyMetrics.from_dict(data.get("latency", {})),
        )
