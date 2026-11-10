#!/usr/bin/env python3
"""Tests for setup.py secret handling. Run: python test_setup.py"""

import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import calendar_config  # noqa: E402
import setup  # noqa: E402


class ClientSecretTest(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        config_dir = Path(self.temp_dir.name)
        config_path = config_dir / "calendar_config.json"
        patches = [
            mock.patch.object(calendar_config, "CONFIG_DIR", config_dir),
            mock.patch.object(calendar_config, "CONFIG_PATH", config_path),
            mock.patch.object(setup, "CONFIG_PATH", config_path),
        ]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        self.addCleanup(self.temp_dir.cleanup)
        self.config_path = config_path

    def run_main(self, *args):
        argv = ["setup.py", *args]
        stdout, stderr = io.StringIO(), io.StringIO()
        with mock.patch.object(sys, "argv", argv), contextlib.redirect_stdout(stdout), \
                contextlib.redirect_stderr(stderr):
            code = setup.main()
        return code, stdout.getvalue(), stderr.getvalue()

    def test_the_client_secret_is_read_from_the_named_variable(self):
        env = {"LIBRAGENT_GOOGLE_CLIENT_SECRET": "\ufeff secret-value\n"}
        with mock.patch.dict(os.environ, env):
            code, _, _ = self.run_main(
                "--client-id", "id.apps.googleusercontent.com",
                "--client-secret-env", "LIBRAGENT_GOOGLE_CLIENT_SECRET",
            )
        self.assertEqual(code, 0)
        saved = json.loads(self.config_path.read_text(encoding="utf-8"))
        self.assertEqual(saved["google"]["client_secret"], "secret-value")

    def test_an_unset_variable_is_an_error_that_names_store_as(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            code, _, stderr = self.run_main(
                "--client-id", "id.apps.googleusercontent.com",
                "--client-secret-env", "LIBRAGENT_GOOGLE_CLIENT_SECRET",
            )
        self.assertEqual(code, 1)
        self.assertIn('storeAs=\\"LIBRAGENT_GOOGLE_CLIENT_SECRET\\"', stderr)


if __name__ == "__main__":
    unittest.main()
