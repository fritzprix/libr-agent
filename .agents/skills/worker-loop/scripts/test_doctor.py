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


if __name__ == "__main__":
    unittest.main()
