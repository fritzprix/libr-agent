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
    """Import setup.py with a stub instagrapi, so the test runs without the package."""
    instagrapi = types.ModuleType("instagrapi")
    instagrapi.Client = object
    exceptions = types.ModuleType("instagrapi.exceptions")
    for name in ("BadPassword", "ClientError", "TwoFactorRequired"):
        setattr(exceptions, name, type(name, (Exception,), {}))
    instagrapi.exceptions = exceptions
    sys.modules.setdefault("instagrapi", instagrapi)
    sys.modules.setdefault("instagrapi.exceptions", exceptions)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import setup

    return setup


setup = import_setup()


class SecretResolutionTest(unittest.TestCase):
    def test_the_password_is_read_from_the_named_variable(self):
        with mock.patch.dict(os.environ, {"IG_PASSWORD": "\ufeff pw\n"}):
            self.assertEqual(setup.read_env_secret("IG_PASSWORD"), "pw")

    def test_an_unset_variable_is_an_error_that_names_store_as(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, 'storeAs="IG_PASSWORD"'):
                setup.read_env_secret("IG_PASSWORD")

    def test_main_exits_with_code_3_when_the_variable_is_unset(self):
        argv = ["setup.py", "--username", "user", "--password-env", "IG_PASSWORD"]
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(sys, "argv", argv), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(setup.main(), 3)

    def test_parser_rejects_the_removed_password_options(self):
        parser = setup.build_parser()
        for removed in (["--password-stdin"], ["--password", "pw"]):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                parser.parse_args(["--username", "user", "--password-env", "IG_PASSWORD"] + removed)


if __name__ == "__main__":
    unittest.main()
