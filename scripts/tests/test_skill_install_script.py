"""Offline tests for skill-creator/scripts/skill.py (#2260)."""

from __future__ import annotations

import base64
import http.server
import io
import json
import os
import sys
import tempfile
import threading
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CREATOR_SCRIPTS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills" / "skill-creator" / "scripts"
if str(CREATOR_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(CREATOR_SCRIPTS_DIR))

import skill

SKILL_MD = """---
name: demo-skill
description: "Use when testing the skill install script end to end."
---
# Demo
"""


def make_skill(root: Path) -> Path:
    skill_dir = root / "demo-skill"
    (skill_dir / "scripts").mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(SKILL_MD)
    (skill_dir / "scripts" / "run.py").write_text("print(1)\n")
    (skill_dir / "__pycache__").mkdir()
    (skill_dir / "__pycache__" / "x.pyc").write_bytes(b"\0")
    return skill_dir


class Handler(http.server.BaseHTTPRequestHandler):
    status = 200
    received: list[dict] = []

    def do_POST(self):
        length = int(self.headers["Content-Length"])
        Handler.received.append(json.loads(self.rfile.read(length)))
        payload = b'{"importedNames":["demo-skill"]}' if Handler.status == 200 else b'{"error":"nope"}'
        self.send_response(Handler.status)
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *args):
        pass


class SkillScriptTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.skill_dir = make_skill(Path(self._tmp.name))
        Handler.status, Handler.received = 200, []
        self.server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        self.url = f"http://127.0.0.1:{self.server.server_port}/install/secret"

    def run_main(self, *argv, env=None):
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.dict(os.environ, env or {}, clear=False), redirect_stdout(out), redirect_stderr(err):
            code = skill.main(list(argv))
        return code, out.getvalue(), err.getvalue()

    def test_collect_files_skips_caches_and_encodes_content(self):
        files = {f["path"]: f for f in skill.collect_files(self.skill_dir)}
        self.assertEqual(set(files), {"SKILL.md", "scripts/run.py"})
        self.assertEqual(base64.b64decode(files["scripts/run.py"]["contentBase64"]), b"print(1)\n")

    def test_validate_reports_ok(self):
        self.assertEqual(self.run_main("validate", str(self.skill_dir))[0], 0)

    def test_deploy_posts_files_to_host(self):
        code, out, _ = self.run_main("deploy", str(self.skill_dir), env={skill.INSTALL_URL_ENV: self.url})
        self.assertEqual(code, 0)
        self.assertIn("demo-skill", out)
        self.assertEqual(
            {f["path"] for f in Handler.received[0]["files"]}, {"SKILL.md", "scripts/run.py"}
        )

    def test_deploy_without_url_fails_before_network(self):
        env = {k: v for k, v in os.environ.items() if k != skill.INSTALL_URL_ENV}
        with mock.patch.dict(os.environ, env, clear=True):
            code, _, err = self.run_main("deploy", str(self.skill_dir))
        self.assertEqual(code, 2)
        self.assertIn(skill.INSTALL_URL_ENV, err)
        self.assertEqual(Handler.received, [])

    def test_deploy_stops_on_invalid_skill(self):
        (self.skill_dir / "SKILL.md").write_text("no frontmatter")
        code, _, _ = self.run_main("deploy", str(self.skill_dir), env={skill.INSTALL_URL_ENV: self.url})
        self.assertEqual(code, 1)
        self.assertEqual(Handler.received, [])

    def test_host_rejection_is_reported_without_leaking_url(self):
        Handler.status = 400
        code, _, err = self.run_main("deploy", str(self.skill_dir), env={skill.INSTALL_URL_ENV: self.url})
        self.assertEqual(code, 1)
        self.assertIn("nope", err)
        self.assertNotIn("secret", err)

    def test_unreachable_host_does_not_leak_url(self):
        url = "http://127.0.0.1:1/install/secret"
        code, _, err = self.run_main("deploy", str(self.skill_dir), env={skill.INSTALL_URL_ENV: url})
        self.assertEqual(code, 1)
        self.assertNotIn("secret", err)


if __name__ == "__main__":
    unittest.main()
