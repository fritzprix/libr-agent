from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CREATOR_SCRIPTS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills" / "skill-creator" / "scripts"
BUNDLED_SKILLS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills"
if str(CREATOR_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(CREATOR_SCRIPTS_DIR))

import frontmatter_yaml  # noqa: E402

load = frontmatter_yaml._fallback_load
Error = frontmatter_yaml.FallbackYAMLError


class FallbackParserTests(unittest.TestCase):
    def test_scalars_and_quotes(self) -> None:
        text = "name: my-skill\ndescription: \"Use when: x\"\nlicense: 'it''s fine'\ncount: 3"
        self.assertEqual(
            load(text),
            {"name": "my-skill", "description": "Use when: x", "license": "it's fine", "count": 3},
        )

    def test_block_scalars(self) -> None:
        text = "name: a\ndescription: |\n  line one\n  line two\nallowed-tools: >-\n  folded\n  text"
        self.assertEqual(
            load(text),
            {"name": "a", "description": "line one\nline two\n", "allowed-tools": "folded text"},
        )

    def test_nested_mapping_and_list(self) -> None:
        self.assertEqual(
            load("name: a\nmetadata:\n  author: me\n  version: 2\nallowed-tools: [Read, 'Bash']"),
            {"name": "a", "metadata": {"author": "me", "version": 2}, "allowed-tools": ["Read", "Bash"]},
        )
        self.assertEqual(load("name: a\nallowed-tools:\n  - Read\n  - Bash"), {"name": "a", "allowed-tools": ["Read", "Bash"]})

    def test_plain_scalar_with_colon_is_rejected(self) -> None:
        with self.assertRaises(Error):
            load("name: a\ndescription: Triggers on: 'science search'")

    def test_unterminated_quote_is_rejected(self) -> None:
        with self.assertRaises(Error):
            load("name: a\ndescription: \"oops")

    def test_matches_pyyaml_on_every_bundled_skill(self) -> None:
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML not installed")
        import re

        for skill_md in sorted(BUNDLED_SKILLS_DIR.glob("*/SKILL.md")):
            match = re.match(r"^---\r?\n(.*?)\r?\n---", skill_md.read_text(encoding="utf-8"), re.DOTALL)
            if not match:
                continue
            with self.subTest(skill=skill_md.parent.name):
                try:
                    expected = yaml.safe_load(match.group(1))
                except yaml.YAMLError:
                    with self.assertRaises(Error):
                        load(match.group(1))
                    continue
                self.assertEqual(load(match.group(1)), expected)


class ValidatorWithoutPyYamlTests(unittest.TestCase):
    def test_validate_skill_runs_when_pyyaml_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            skill_dir = Path(tmp) / "demo-skill"
            skill_dir.mkdir()
            (skill_dir / "SKILL.md").write_text(
                "---\nname: demo-skill\ndescription: Use when the user needs a demo.\n---\n\n# Demo\n",
                encoding="utf-8",
            )
            code = (
                "import sys, runpy; sys.modules['yaml'] = None; "
                f"sys.argv = ['validate_skill.py', {str(skill_dir)!r}, '--strict']; "
                f"runpy.run_path({str(CREATOR_SCRIPTS_DIR / 'validate_skill.py')!r}, run_name='__main__')"
            )
            result = subprocess.run(
                [sys.executable, "-c", code],
                capture_output=True,
                text=True,
                cwd=str(CREATOR_SCRIPTS_DIR),
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
