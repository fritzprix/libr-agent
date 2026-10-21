"""Offline unit tests for deploy_skill.py and github_fetch.py (#2181)."""

from __future__ import annotations

import http.client
import io
import json
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEPLOYER_SCRIPTS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills" / "skill-deployer" / "scripts"
CREATOR_SCRIPTS_DIR = REPO_ROOT / "src-tauri" / "bundled_skills" / "skill-creator" / "scripts"
for script_dir in (DEPLOYER_SCRIPTS_DIR, CREATOR_SCRIPTS_DIR):
    if str(script_dir) not in sys.path:
        sys.path.insert(0, str(script_dir))

import deploy_skill
import github_fetch
import validate_skill


def make_zip_bytes(top_dir: str, files: dict[str, bytes | str]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for rel_path, content in files.items():
            data = content.encode("utf-8") if isinstance(content, str) else content
            zf.writestr(f"{top_dir}/{rel_path}", data)
    return buf.getvalue()


VALID_PROMPT_SKILL_MD = """---
name: alpha-skill
description: "Use when testing alpha skill installation from GitHub."
---

# Alpha Skill

Run alpha steps carefully.
"""

VALID_SCRIPT_SKILL_MD = """---
name: beta-runner
description: "Use when testing beta script skill installation from GitHub."
---

# Beta Runner

Execute scripts/run_beta.py to process items.
"""


def run_cli(args: list[str]) -> tuple[int, str, str]:
    out_buf, err_buf = io.StringIO(), io.StringIO()
    with (
        mock.patch.object(sys, "argv", ["deploy_skill.py", *args]),
        redirect_stdout(out_buf),
        redirect_stderr(err_buf),
    ):
        try:
            code = deploy_skill.main()
        except SystemExit as exc:
            code = int(exc.code) if exc.code is not None else 0
    return code, out_buf.getvalue(), err_buf.getvalue()


class DeploySkillUnitTests(unittest.TestCase):
    def test_stripped_root_files_cover_validator_forbidden_files(self) -> None:
        forbidden_lower = {name.lower() for name in validate_skill.FORBIDDEN_SKILL_FILES}
        self.assertTrue(
            forbidden_lower.issubset(github_fetch.STRIPPED_SKILL_ROOT_FILES_LOWER),
            f"Missing from STRIPPED_SKILL_ROOT_FILES_LOWER: {forbidden_lower - github_fetch.STRIPPED_SKILL_ROOT_FILES_LOWER}",
        )

    def test_parse_github_url_variants_and_conflicts(self) -> None:
        # 1. Repo root URL
        p1 = github_fetch.parse_github_url("https://github.com/acme/my-skills.git")
        self.assertEqual(p1.owner, "acme")
        self.assertEqual(p1.repo, "my-skills")
        self.assertIsNone(p1.explicit_ref)
        self.assertIsNone(p1.explicit_subpath)
        self.assertEqual(p1.tree_segments, ())

        # 2. Query params
        p2 = github_fetch.parse_github_url(
            "https://github.com/acme/my-skills?ref=v1.2.0&path=skills/alpha-skill"
        )
        self.assertEqual(p2.explicit_ref, "v1.2.0")
        self.assertEqual(p2.explicit_subpath, "skills/alpha-skill")

        # 3. Tree URL
        p3 = github_fetch.parse_github_url(
            "https://github.com/acme/my-skills/tree/release/1.0/skills/alpha-skill"
        )
        self.assertEqual(p3.tree_segments, ("release", "1.0", "skills", "alpha-skill"))

        # 4. Matching --ref with /tree/<ref>/<subpath>
        p3_match = github_fetch.parse_github_url(
            "https://github.com/acme/my-skills/tree/main/skills/alpha-skill",
            ref_override="main",
        )
        self.assertEqual(p3_match.explicit_ref, "main")
        self.assertEqual(p3_match.explicit_subpath, "skills/alpha-skill")

        # 5. Conflicting --ref with /tree/<other-ref>/<subpath> is rejected
        with self.assertRaises(github_fetch.GitHubDeployError) as ctx:
            github_fetch.parse_github_url(
                "https://github.com/acme/my-skills/tree/main/skills/alpha-skill",
                ref_override="v1",
            )
        self.assertEqual(ctx.exception.code, "DEPLOY-GITHUB-001")
        self.assertIn("Conflicting --ref 'v1'", ctx.exception.message)

        # 6. Blob URL pointing to SKILL.md
        p4 = github_fetch.parse_github_url(
            "https://github.com/acme/my-skills/blob/main/skills/alpha-skill/SKILL.md"
        )
        self.assertEqual(p4.tree_segments, ("main", "skills", "alpha-skill"))

        # 7. Commit URL and conflicting --ref
        sha = "0123456789abcdef0123456789abcdef01234567"
        p5 = github_fetch.parse_github_url(f"https://github.com/acme/my-skills/commit/{sha}")
        self.assertEqual(p5.explicit_ref, sha)
        with self.assertRaises(github_fetch.GitHubDeployError):
            github_fetch.parse_github_url(
                f"https://github.com/acme/my-skills/commit/{sha}",
                ref_override="main",
            )

        # 8. Reject non-github host and path traversal
        with self.assertRaises(github_fetch.GitHubDeployError):
            github_fetch.parse_github_url("https://gitlab.com/acme/my-skills")
        with self.assertRaises(github_fetch.GitHubDeployError):
            github_fetch.parse_github_url(
                "https://github.com/acme/my-skills?path=../../etc/passwd"
            )

    def test_resolve_github_spec_disambiguates_slash_branches(self) -> None:
        sha_main = "1111111111111111111111111111111111111111"
        sha_rel = "2222222222222222222222222222222222222222"
        fake_refs = {
            "HEAD": sha_main,
            "main": sha_main,
            "release/1.0": sha_rel,
        }
        with mock.patch.object(
            github_fetch,
            "query_git_ls_remote",
            return_value=(sha_main, "main", fake_refs),
        ):
            parsed = github_fetch.parse_github_url(
                "https://github.com/acme/my-skills/tree/release/1.0/skills/alpha-skill"
            )
            spec = github_fetch.resolve_github_spec(parsed)
            self.assertEqual(spec.ref, "release/1.0")
            self.assertEqual(spec.commit, sha_rel)
            self.assertEqual(spec.subpath, "skills/alpha-skill")

    def test_single_skill_at_repo_root_strips_readme_and_github_dir(self) -> None:
        sha = "cccccccccccccccccccccccccccccccccccccccc"
        zip_root_skill = make_zip_bytes(
            f"alpha-skill-{sha}",
            {
                "SKILL.md": VALID_PROMPT_SKILL_MD,
                "README.md": "# Repo README that must be stripped\n",
                "CHANGELOG.md": "# Repo changelog\n",
                ".github/workflows/ci.yml": "name: CI\n",
                ".github/scripts/ci_helper.py": "print('ci only')\n",
                "LICENSE.txt": "MIT\n",
            },
        )
        with (
            tempfile.TemporaryDirectory() as tmp_str,
            mock.patch.object(
                github_fetch,
                "query_git_ls_remote",
                return_value=(sha, "main", {"HEAD": sha, "main": sha}),
            ),
            mock.patch.object(
                github_fetch,
                "_download_commit_zip",
                side_effect=lambda _spec, path: path.write_bytes(zip_root_skill),
            ),
        ):
            tmp = Path(tmp_str)
            workspace = tmp / "workspaces" / "session-1"
            workspace.mkdir(parents=True)

            # Should install without requiring --approve-scripts (since .github/scripts/ is stripped)
            # and pass strict validation (since README.md and CHANGELOG.md are stripped).
            code, _out, err = run_cli(
                [
                    "https://github.com/acme/alpha-skill",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 0, err)
            deployed = workspace / ".libragent" / "skills" / "alpha-skill"
            self.assertTrue((deployed / "SKILL.md").is_file())
            self.assertTrue((deployed / "LICENSE.txt").is_file())
            self.assertFalse((deployed / "README.md").exists())
            self.assertFalse((deployed / "CHANGELOG.md").exists())
            self.assertFalse((deployed / ".github").exists())

    def test_local_skill_install_and_update_with_readme_in_source(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_str:
            tmp = Path(tmp_str)
            source_dir = tmp / "alpha-skill"
            source_dir.mkdir(parents=True)
            (source_dir / "SKILL.md").write_text(VALID_PROMPT_SKILL_MD, encoding="utf-8")

            workspace = tmp / "project-workspace"
            workspace.mkdir(parents=True)

            code, _out, err = run_cli(
                [
                    str(source_dir),
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 0, err)
            deployed = workspace / ".libragent" / "skills" / "alpha-skill"
            self.assertTrue((deployed / "SKILL.md").is_file())

            # Add README.md to local source and update
            (source_dir / "README.md").write_text("# Local README\n", encoding="utf-8")
            (source_dir / "SKILL.md").write_text(
                VALID_PROMPT_SKILL_MD.replace("Run alpha steps carefully.", "Updated local body."),
                encoding="utf-8",
            )
            code, _out, err = run_cli(
                [
                    "--update",
                    "alpha-skill",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 0, err)
            self.assertFalse((deployed / "README.md").exists())
            self.assertIn("Updated local body.", (deployed / "SKILL.md").read_text("utf-8"))

    def test_multi_skill_selection_approval_provenance_update_and_remove(self) -> None:
        sha1 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        sha2 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

        # Repo named "libragent-skills" with "skills/" subdirectory to ensure no false SKILL-PATH-LEGACY-GLOBAL
        zip_v1 = make_zip_bytes(
            f"libragent-skills-{sha1}",
            {
                "skills/alpha-skill/SKILL.md": VALID_PROMPT_SKILL_MD,
                "skills/beta-runner/SKILL.md": VALID_SCRIPT_SKILL_MD,
                "skills/beta-runner/scripts/run_beta.py": "#!/usr/bin/env python3\nprint('v1')\n",
            },
        )
        zip_v2 = make_zip_bytes(
            f"libragent-skills-{sha2}",
            {
                "skills/alpha-skill/SKILL.md": VALID_PROMPT_SKILL_MD.replace(
                    "Run alpha steps carefully.", "Run alpha steps v2."
                ),
                "skills/beta-runner/SKILL.md": VALID_SCRIPT_SKILL_MD,
                "skills/beta-runner/scripts/run_beta.py": "#!/usr/bin/env python3\nprint('v2')\n",
            },
        )

        current_sha = {"val": sha1}
        current_zip = {"val": zip_v1}

        def fake_ls_remote(_url: str):
            s = current_sha["val"]
            return s, "main", {"HEAD": s, "main": s}

        def fake_download(spec: github_fetch.ResolvedGitHubSpec, archive_path: Path):
            self.assertEqual(spec.commit, current_sha["val"])
            archive_path.write_bytes(current_zip["val"])

        with (
            tempfile.TemporaryDirectory() as tmp_str,
            mock.patch.object(github_fetch, "query_git_ls_remote", side_effect=fake_ls_remote),
            mock.patch.object(github_fetch, "_download_commit_zip", side_effect=fake_download),
        ):
            tmp = Path(tmp_str)
            workspace = tmp / "workspaces" / "session-1"
            workspace.mkdir(parents=True)

            # 1. Inspect multi-skill repo -> exits 0 and lists both skills
            code, out, err = run_cli(["https://github.com/acme/libragent-skills", "--inspect"])
            self.assertEqual(code, 0, err)
            self.assertIn("alpha-skill", out)
            self.assertIn("beta-runner", out)

            # 2. Deploy multi-skill repo without --skill -> exits 3 (DEPLOY-SELECT-001)
            code, _out, err = run_cli(
                [
                    "https://github.com/acme/libragent-skills",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 3)
            self.assertIn("DEPLOY-SELECT-001", err)

            # 3. Deploy prompt-only skill (alpha-skill) -> succeeds without --approve-scripts
            code, _out, err = run_cli(
                [
                    "https://github.com/acme/libragent-skills",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                    "--skill",
                    "alpha-skill",
                ]
            )
            self.assertEqual(code, 0, err)
            alpha_target = workspace / ".libragent" / "skills" / "alpha-skill"
            self.assertTrue((alpha_target / "SKILL.md").is_file())
            prov = json.loads((alpha_target / ".skill-provenance.json").read_text("utf-8"))
            self.assertEqual(prov["sourceType"], "github")
            self.assertEqual(prov["commit"], sha1)
            self.assertEqual(prov["subpath"], "skills/alpha-skill")
            self.assertFalse(prov["hasScripts"])

            # 4. Deploy script-containing skill (beta-runner) WITHOUT --approve-scripts -> blocked (code 4)
            code, _out, err = run_cli(
                [
                    "https://github.com/acme/libragent-skills",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                    "--skill",
                    "beta-runner",
                ]
            )
            self.assertEqual(code, 4)
            self.assertIn("DEPLOY-APPROVAL-001", err)
            beta_target = workspace / ".libragent" / "skills" / "beta-runner"
            self.assertFalse(beta_target.exists())

            # 5. Deploy beta-runner WITH --approve-scripts -> succeeds
            code, _out, err = run_cli(
                [
                    "https://github.com/acme/libragent-skills",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                    "--skill",
                    "beta-runner",
                    "--approve-scripts",
                ]
            )
            self.assertEqual(code, 0, err)
            self.assertTrue((beta_target / "SKILL.md").is_file())
            beta_prov = json.loads((beta_target / ".skill-provenance.json").read_text("utf-8"))
            self.assertEqual(beta_prov["commit"], sha1)
            self.assertEqual(beta_prov["scripts"], ["scripts/run_beta.py"])

            # 6. Update alpha-skill to sha2 via --update
            current_sha["val"] = sha2
            current_zip["val"] = zip_v2
            code, _out, err = run_cli(
                [
                    "--update",
                    "alpha-skill",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 0, err)
            self.assertIn("Run alpha steps v2.", (alpha_target / "SKILL.md").read_text("utf-8"))
            updated_prov = json.loads(
                (alpha_target / ".skill-provenance.json").read_text("utf-8")
            )
            self.assertEqual(updated_prov["commit"], sha2)
            self.assertEqual(updated_prov["installedAt"], prov["installedAt"])

            # 7. Reject invalid skill name on --remove
            code, _out, err = run_cli(
                [
                    "--remove",
                    "../alpha-skill",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 2)
            self.assertIn("DEPLOY-ARGS-004", err)

            # 8. Remove alpha-skill via --remove
            code, _out, err = run_cli(
                [
                    "--remove",
                    "alpha-skill",
                    "--scope",
                    "workspace",
                    "--workspace",
                    str(workspace),
                ]
            )
            self.assertEqual(code, 0, err)
            self.assertFalse(alpha_target.exists())

    def test_truncated_zip_download_falls_back_to_git_fetch(self) -> None:
        spec = github_fetch.ResolvedGitHubSpec(
            requested_url="https://github.com/acme/alpha-skill",
            owner="acme",
            repo="alpha-skill",
            ref="main",
            commit="a" * 40,
            subpath=None,
        )

        def fake_git_fetch(_spec: github_fetch.ResolvedGitHubSpec, repo_dest: Path) -> None:
            repo_dest.mkdir(parents=True, exist_ok=True)
            (repo_dest / "SKILL.md").write_text(VALID_PROMPT_SKILL_MD, encoding="utf-8")

        with (
            tempfile.TemporaryDirectory() as tmp,
            mock.patch.object(
                github_fetch,
                "_download_commit_zip",
                side_effect=http.client.IncompleteRead(b"partial"),
            ),
            mock.patch.object(
                github_fetch, "_git_fetch_commit_fallback", side_effect=fake_git_fetch
            ) as git_fallback,
        ):
            repo_root = github_fetch.fetch_github_repo_at_commit(spec, Path(tmp))

            git_fallback.assert_called_once()
            self.assertTrue((repo_root / "SKILL.md").is_file())


if __name__ == "__main__":
    unittest.main(verbosity=2)
