"""Unit tests for the Torque Python SDK."""

import unittest
from unittest.mock import patch, MagicMock
from torque_client.models import ServerStats
from torque_client.client import TorqueAdminClient


class TestTorqueModels(unittest.TestCase):
    def test_server_stats_parsing(self):
        sample_json = {
            "requests": {
                "total": 12500,
                "2xx": 12000,
                "3xx": 300,
                "4xx": 150,
                "5xx": 50
            },
            "connections": 42,
            "bytes": {
                "sent": 2048000,
                "received": 1024000
            },
            "latency": {
                "mean": 2.45,
                "p50": 1.82,
                "p90": 4.15,
                "p99": 8.70
            }
        }

        stats = ServerStats.from_dict(sample_json)

        self.assertEqual(stats.requests.total, 12500)
        self.assertEqual(stats.requests.status_2xx, 12000)
        self.assertEqual(stats.requests.status_5xx, 50)
        self.assertEqual(stats.connections, 42)
        self.assertEqual(stats.bytes.sent_bytes, 2048000)
        self.assertAlmostEqual(stats.bytes.sent_kb, 2000.0)
        self.assertAlmostEqual(stats.latency.p50_ms, 1.82)
        self.assertAlmostEqual(stats.latency.p99_ms, 8.70)


class TestTorqueAdminClient(unittest.TestCase):
    def setUp(self):
        self.client = TorqueAdminClient("http://127.0.0.1:9090")

    @patch("urllib.request.urlopen")
    def test_is_alive_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.status = 200
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        self.assertTrue(self.client.is_alive())

    @patch("urllib.request.urlopen")
    def test_get_stats_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.status = 200
        mock_resp.read.return_value = b'{"requests":{"total":100,"2xx":100,"3xx":0,"4xx":0,"5xx":0},"connections":5,"bytes":{"sent":1024,"received":512},"latency":{"mean":1.2,"p50":1.0,"p90":1.8,"p99":2.5}}'
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        stats = self.client.get_stats()
        self.assertEqual(stats.requests.total, 100)
        self.assertEqual(stats.connections, 5)


if __name__ == "__main__":
    unittest.main()
