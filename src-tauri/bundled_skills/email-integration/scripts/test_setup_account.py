#!/usr/bin/env python3
"""Tests for setup_account.py secret handling. Run: python test_setup_account.py"""

import contextlib
import io
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import setup_account  # noqa: E402


class PasswordResolutionTest(unittest.TestCase):
    def parse(self, *extra):
        return setup_account.build_parser().parse_args(["--email", "user@example.com", *extra])

    def test_the_password_is_read_from_the_named_variable(self):
        args = self.parse("--password-env", "LIBRAGENT_EMAIL_PASSWORD")
        with mock.patch.dict(os.environ, {"LIBRAGENT_EMAIL_PASSWORD": "\ufeff app-pw\n"}):
            self.assertEqual(setup_account.resolve_password(args), "app-pw")

    def test_an_unset_variable_exits_with_code_3_and_names_store_as(self):
        args = self.parse("--password-env", "LIBRAGENT_EMAIL_PASSWORD")
        stderr = io.StringIO()
        with mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as raised:
                setup_account.resolve_password(args)
        self.assertEqual(raised.exception.code, 3)
        self.assertIn('storeAs="LIBRAGENT_EMAIL_PASSWORD"', stderr.getvalue())

    def test_parser_rejects_password_stdin(self):
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            self.parse("--password-stdin")

    def test_setup_does_not_import_getpass(self):
        self.assertFalse(hasattr(setup_account, "getpass"))


if __name__ == "__main__":
    unittest.main()
