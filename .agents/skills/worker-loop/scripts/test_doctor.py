#!/usr/bin/env python3
"""Tests for doctor.py diagnostic checks."""
import unittest
from unittest.mock import MagicMock, patch

import doctor


class DoctorTest(unittest.TestCase):
    @patch("doctor.subprocess.run")
    def test_check_success(self, mock_run):
        mock_run.return_value = MagicMock(returncode=0, stdout="v1.0.0", stderr="")
        self.assertTrue(doctor.check("test-cmd", ["dummy"]))

    @patch("doctor.subprocess.run")
    def test_check_failure(self, mock_run):
        mock_run.return_value = MagicMock(returncode=1, stdout="", stderr="error occurred")
        self.assertFalse(doctor.check("test-cmd", ["dummy"]))

    @patch("doctor.subprocess.run")
    def test_check_file_not_found(self, mock_run):
        mock_run.side_effect = FileNotFoundError()
        self.assertFalse(doctor.check("nonexistent", ["nonexistent"]))

    @patch("doctor.urllib.request.urlopen")
    def test_check_network_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.status = 200
        mock_resp.__enter__.return_value = mock_resp
        mock_urlopen.return_value = mock_resp
        self.assertTrue(doctor.check_network())
        self.assertEqual(mock_urlopen.call_args.kwargs.get("timeout"), doctor.NETWORK_TIMEOUT_SECONDS)

    @patch("doctor.urllib.request.urlopen")
    def test_check_network_failure(self, mock_urlopen):
        mock_urlopen.side_effect = Exception("network unreachable")
        self.assertFalse(doctor.check_network())

    @patch("doctor.urllib.request.urlopen")
    def test_check_network_rate_limit_403(self, mock_urlopen):
        mock_urlopen.side_effect = doctor.urllib.error.HTTPError("https://api.github.com", 403, "rate limit", {}, None)
        self.assertTrue(doctor.check_network())

    @patch("doctor.urllib.request.urlopen")
    def test_check_network_rate_limit_429(self, mock_urlopen):
        mock_urlopen.side_effect = doctor.urllib.error.HTTPError("https://api.github.com", 429, "too many requests", {}, None)
        self.assertTrue(doctor.check_network())

    @patch("doctor.urllib.request.urlopen")
    def test_check_network_http_500_failure(self, mock_urlopen):
        mock_urlopen.side_effect = doctor.urllib.error.HTTPError("https://api.github.com", 500, "internal server error", {}, None)
        self.assertFalse(doctor.check_network())

    @patch("doctor.subprocess.run")
    def test_check_push_permission_success(self, mock_run):
        mock_run.return_value = MagicMock(returncode=0, stderr="")
        self.assertTrue(doctor.check_push_permission())

    @patch("doctor.subprocess.run")
    def test_check_push_permission_non_fast_forward(self, mock_run):
        mock_run.return_value = MagicMock(returncode=1, stderr="[rejected] (non-fast-forward)")
        self.assertTrue(doctor.check_push_permission())

    @patch("doctor.subprocess.run")
    def test_check_push_permission_failure(self, mock_run):
        mock_run.return_value = MagicMock(returncode=1, stderr="fatal: remote error: access denied")
        self.assertFalse(doctor.check_push_permission())


if __name__ == "__main__":
    unittest.main()
