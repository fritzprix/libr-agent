#!/usr/bin/env python3
"""Tests for setup.py secret handling. Run: python test_setup.py"""

import contextlib
import io
import os
import sys
import types
import unittest
from pathlib import Path
from unittest import mock


def import_setup():
    """Import setup.py with a stub telethon, so the test runs without the package."""
    telethon = types.ModuleType("telethon")
    telethon.TelegramClient = object
    errors = types.ModuleType("telethon.errors")
    for name in (
        "AuthRestartError",
        "SessionPasswordNeededError",
        "PhoneCodeInvalidError",
        "PhoneCodeExpiredError",
        "PasswordHashInvalidError",
        "FloodWaitError",
        "RPCError",
    ):
        setattr(errors, name, type(name, (Exception,), {}))
    telethon.errors = errors
    sys.modules.setdefault("telethon", telethon)
    sys.modules.setdefault("telethon.errors", errors)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import setup

    return setup


setup = import_setup()


class SecretResolutionTest(unittest.TestCase):
    def test_secrets_are_read_from_the_named_environment_variables(self):
        with mock.patch.dict(
            os.environ,
            {"TELEGRAM_API_HASH": "hash-value", "TELEGRAM_CODE": " 12345\n", "TELEGRAM_2FA": "\ufeffpw"},
        ):
            self.assertEqual(setup.resolve_secret(None, "TELEGRAM_API_HASH", "API hash"), ("hash-value", None))
            self.assertEqual(setup.resolve_secret(None, "TELEGRAM_CODE", "verification code"), ("12345", None))
            self.assertEqual(setup.resolve_secret(None, "TELEGRAM_2FA", "2FA password"), ("pw", None))

    def test_an_unset_variable_is_an_error_that_names_store_as(self):
        stderr = io.StringIO()
        with mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stderr(stderr):
            self.assertEqual(setup.resolve_secret(None, "TELEGRAM_CODE", "verification code"), ("", 1))
        self.assertIn('storeAs=\\"TELEGRAM_CODE\\"', stderr.getvalue())

    def test_parser_has_env_options_and_no_stdin_options(self):
        parser = setup.build_parser()
        args = parser.parse_args(
            ["--action", "sign_in", "--api-hash-env", "TELEGRAM_API_HASH", "--code-env", "TELEGRAM_CODE"]
        )
        self.assertEqual(args.api_hash_env, "TELEGRAM_API_HASH")
        self.assertEqual(args.code_env, "TELEGRAM_CODE")
        for removed in ("--code-stdin", "--password-stdin"):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                parser.parse_args(["--action", "sign_in", removed])


if __name__ == "__main__":
    unittest.main()
