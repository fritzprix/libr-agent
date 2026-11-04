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
    """Import setup.py with stub twikit modules, so the test runs without the package."""
    twikit = types.ModuleType("twikit")
    twikit.Client = object
    patches = types.ModuleType("twikit_patches")
    patches.apply_twikit_patches = lambda: None
    sys.modules.setdefault("twikit", twikit)
    sys.modules.setdefault("twikit_patches", patches)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import setup

    return setup


setup = import_setup()
BASE_ARGS = ["--username", "user", "--email", "user@example.com"]


class SecretResolutionTest(unittest.TestCase):
    def test_cookie_and_password_secrets_are_read_from_the_named_variables(self):
        args = setup.build_parser().parse_args(
            BASE_ARGS
            + [
                "--password-env", "X_PASSWORD",
                "--totp-secret-env", "X_TOTP_SECRET",
                "--auth-token-env", "X_AUTH_TOKEN",
                "--ct0-env", "X_CT0",
            ]
        )
        env = {"X_PASSWORD": " pw\n", "X_TOTP_SECRET": "totp", "X_AUTH_TOKEN": "\ufefftoken", "X_CT0": "ct0"}
        with mock.patch.dict(os.environ, env):
            self.assertEqual(setup.resolve_auth_inputs(args), ("pw", "totp", "token", "ct0"))

    def test_options_that_are_not_given_resolve_to_none(self):
        args = setup.build_parser().parse_args(BASE_ARGS + ["--password-env", "X_PASSWORD"])
        with mock.patch.dict(os.environ, {"X_PASSWORD": "pw"}):
            self.assertEqual(setup.resolve_auth_inputs(args), ("pw", None, None, None))

    def test_an_unset_variable_is_an_error_that_names_store_as(self):
        args = setup.build_parser().parse_args(BASE_ARGS + ["--ct0-env", "X_CT0"])
        with mock.patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, 'storeAs="X_CT0"'):
                setup.resolve_auth_inputs(args)

    def test_parser_rejects_the_removed_secret_options(self):
        parser = setup.build_parser()
        for removed in (["--password-stdin"], ["--cookie-login"], ["--password", "pw"], ["--auth-token", "t"],
                        ["--ct0", "c"], ["--totp-secret", "-"]):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                parser.parse_args(BASE_ARGS + removed)

    def test_the_recovery_command_uses_env_names(self):
        command = setup.build_cookie_setup_command()
        self.assertIn("--auth-token-env X_AUTH_TOKEN --ct0-env X_CT0", command)


if __name__ == "__main__":
    unittest.main()
