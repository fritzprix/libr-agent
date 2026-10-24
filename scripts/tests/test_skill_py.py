"""Offline tests for skill-creator/scripts/skill.py (#2260)."""

from __future__ import annotations

import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CREATOR_SCRIPTS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills" / "skill-creator" / "scripts"
if str(CREATOR_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(CREATOR_SCRIPTS_DIR))

import skill
from unittest import mock


class SkillPyTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        root = Path(self._tmp.name).resolve()
        self.data_dir = root / "data"
        (self.data_dir / "user_skills").mkdir(parents=True)
        (self.data_dir / "system_skills").mkdir()
        self.work = root / "work"
        self.work.mkdir()

    def run_skill(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = skill.main(["--data-dir", str(self.data_dir), *argv])
        return code, out.getvalue(), err.getvalue()

    def new_skill(self, name="demo-skill"):
        self.assertEqual(self.run_skill("new", name, "--to", str(self.work))[0], 0)
        return self.work / name

    def test_new_template_passes_validation(self):
        path = self.new_skill()
        self.assertTrue((path / "SKILL.md").is_file())
        self.assertEqual(self.run_skill("validate", str(path))[0], 0)

    def test_new_rejects_bad_names_and_existing_dirs(self):
        self.assertEqual(self.run_skill("new", "../evil", "--to", str(self.work))[0], 1)
        self.assertEqual(self.run_skill("new", "Bad_Name", "--to", str(self.work))[0], 1)
        self.new_skill()
        self.assertEqual(self.run_skill("new", "demo-skill", "--to", str(self.work))[0], 1)

    def test_deploy_copies_into_user_skills_and_overwrites(self):
        path = self.new_skill()
        (path / "scripts").mkdir()
        (path / "scripts" / "run.py").write_text("print(1)\n")
        (path / "__pycache__").mkdir()
        (path / "__pycache__" / "x.pyc").write_bytes(b"\0")
        self.assertEqual(self.run_skill("deploy", str(path))[0], 0)
        target = self.data_dir / "user_skills" / "demo-skill"
        self.assertTrue((target / "scripts" / "run.py").is_file())
        self.assertFalse((target / "__pycache__").exists())

        (path / "scripts" / "run.py").write_text("print(2)\n")
        self.assertEqual(self.run_skill("deploy", str(path))[0], 0)
        self.assertEqual((target / "scripts" / "run.py").read_text(), "print(2)\n")

    def test_data_dir_follows_the_app_resolution_order(self):
        env = {"LIBRAGENT_DATA_DIR": "/tmp/instance-a"}
        with mock.patch.dict(os.environ, env):
            self.assertEqual(skill.default_data_dir(), Path("/tmp/instance-a"))
        clean = {k: v for k, v in os.environ.items() if k not in ("LIBRAGENT_DATA_DIR", "LIBRAGENT_PROFILE")}
        with mock.patch.dict(os.environ, clean, clear=True), mock.patch("platform.system", return_value="Linux"):
            os.environ["XDG_DATA_HOME"] = "/tmp/xdg"
            self.assertEqual(skill.default_data_dir(), Path("/tmp/xdg/com.fritzprix.libragent"))
            os.environ["LIBRAGENT_PROFILE"] = "demo"
            self.assertEqual(skill.default_data_dir(), Path("/tmp/xdg/com.fritzprix.libragent-demo"))

    def test_env_data_dir_redirects_deploy(self):
        path = self.new_skill()
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.dict(os.environ, {"LIBRAGENT_DATA_DIR": str(self.data_dir)}), redirect_stdout(out), redirect_stderr(err):
            self.assertEqual(skill.main(["deploy", str(path)]), 0)
        self.assertTrue((self.data_dir / "user_skills" / "demo-skill" / "SKILL.md").is_file())

    def test_symlinks_are_not_copied_on_deploy_or_clone(self):
        outside = Path(self._tmp.name) / "secret.txt"
        outside.write_text("secret")
        path = self.new_skill()
        os.symlink(outside, path / "link.txt")
        self.assertEqual(self.run_skill("deploy", str(path))[0], 0)
        target = self.data_dir / "user_skills" / "demo-skill"
        self.assertFalse((target / "link.txt").exists() or (target / "link.txt").is_symlink())
        os.symlink(outside, target / "link2.txt")
        dest = self.work / "cl"
        dest.mkdir()
        self.assertEqual(self.run_skill("clone", "demo-skill", "--to", str(dest))[0], 0)
        self.assertFalse((dest / "demo-skill" / "link2.txt").is_symlink())

    def test_failed_deploy_leaves_no_staging_dirs(self):
        path = self.new_skill()
        self.run_skill("deploy", str(path))
        (path / "SKILL.md").write_text("no frontmatter")
        self.run_skill("deploy", str(path))
        self.assertEqual([p.name for p in (self.data_dir / "user_skills").iterdir()], ["demo-skill"])

    def test_deploy_refuses_invalid_skill_and_keeps_existing(self):
        path = self.new_skill()
        self.assertEqual(self.run_skill("deploy", str(path))[0], 0)
        target = self.data_dir / "user_skills" / "demo-skill"
        before = (target / "SKILL.md").read_text()
        (path / "SKILL.md").write_text("no frontmatter")
        self.assertEqual(self.run_skill("deploy", str(path))[0], 1)
        self.assertEqual((target / "SKILL.md").read_text(), before)

    def test_deploy_writes_only_to_user_skills_name(self):
        path = self.new_skill()
        self.assertEqual(self.run_skill("deploy", str(path))[0], 0)
        self.assertEqual([p.name for p in (self.data_dir / "user_skills").iterdir()], ["demo-skill"])
        self.assertEqual(sorted(p.name for p in self.data_dir.iterdir()), ["system_skills", "user_skills"])

    def test_deploy_refuses_to_redeploy_from_the_user_skills_folder(self):
        path = self.new_skill()
        self.run_skill("deploy", str(path))
        deployed = self.data_dir / "user_skills" / "demo-skill"
        self.assertEqual(self.run_skill("deploy", str(deployed))[0], 1)

    def test_deploy_requires_existing_data_dir(self):
        path = self.new_skill()
        code, _, err = self.run_skill("--data-dir", str(self.data_dir / "missing"), "deploy", str(path))
        self.assertEqual(code, 1)
        self.assertFalse((self.data_dir / "missing").exists())

    def test_clone_user_then_system_and_never_touches_source(self):
        src = self.new_skill("alpha")
        self.run_skill("deploy", str(src))
        sys_skill = self.data_dir / "system_skills" / "beta"
        sys_skill.mkdir()
        (sys_skill / "SKILL.md").write_text((src / "SKILL.md").read_text().replace("alpha", "beta"))
        dest = self.work / "clones"
        dest.mkdir()
        for name in ("alpha", "beta"):
            self.assertEqual(self.run_skill("clone", name, "--to", str(dest))[0], 0)
            self.assertTrue((dest / name / "SKILL.md").is_file())
        self.assertEqual(self.run_skill("clone", "alpha", "--to", str(dest))[0], 1)
        self.assertEqual(self.run_skill("clone", "missing", "--to", str(dest))[0], 1)
        self.assertEqual(self.run_skill("clone", "../x", "--to", str(dest))[0], 1)


if __name__ == "__main__":
    unittest.main()
