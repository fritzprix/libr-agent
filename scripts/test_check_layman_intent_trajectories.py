"""Tests for check-layman-intent-trajectories.py and its critical-path rules.

Run: python3 -m unittest scripts/test_check_layman_intent_trajectories.py
"""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DATASET = REPO / "benchmarks" / "harbor" / "libragent-layman-intent"
SHARED = REPO / "benchmarks" / "harbor" / "layman_intent_critical_path.py"

_SPEC = importlib.util.spec_from_file_location(
    "layman_checker", Path(__file__).with_name("check-layman-intent-trajectories.py")
)
assert _SPEC is not None and _SPEC.loader is not None
checker = importlib.util.module_from_spec(_SPEC)
# dataclasses resolve annotations through sys.modules.
sys.modules[_SPEC.name] = checker
_SPEC.loader.exec_module(checker)

from layman_intent_critical_path import TASKS, grade_calls, load_calls, runs_script


def _step(*calls: tuple[str, str, object, bool | None]) -> dict[str, object]:
    """Agent step with (call_id, name, arguments, tool_error) tuples."""
    return {
        "source": "agent",
        "tool_calls": [
            {"tool_call_id": call_id, "function_name": name, "arguments": arguments}
            for call_id, name, arguments, _ in calls
        ],
        "observation": {
            "results": [
                {
                    "source_call_id": call_id,
                    "content": "",
                    **({} if error is None else {"extra": {"tool_error": error}}),
                }
                for call_id, _, _, error in calls
            ]
        },
    }


def _grade(task_id: str, *steps: dict[str, object]) -> list[str]:
    return grade_calls(TASKS[task_id], load_calls({"steps": list(steps)}))


class CriticalPathTests(unittest.TestCase):
    def test_pass_needs_every_step_with_last_call_succeeding(self) -> None:
        self.assertEqual(
            _grade(
                "T07",
                _step(("a", "tool__registerServer", {}, True)),
                _step(("b", "tool__registerServer", {}, False)),
                _step(("c", "tool__verifyServer", {}, False)),
            ),
            [],
        )

    def test_missing_step_and_failed_last_call(self) -> None:
        failures = _grade(
            "T07",
            _step(("a", "tool__registerServer", {}, False)),
            _step(("b", "tool__registerServer", {}, True)),
        )
        self.assertEqual(
            failures,
            ["last __registerServer failed", "missing __verifyServer"],
        )

    def test_call_without_recorded_result_does_not_pass(self) -> None:
        self.assertEqual(
            _grade("T01", _step(("a", "task__scheduleCallback", {}, None))),
            ["last __scheduleCallback has no result"],
        )

    def test_script_steps_match_on_arguments(self) -> None:
        unrelated = _step(("a", "workspace__runShell", {"command": "ls"}, False))
        self.assertEqual(
            _grade("T08", unrelated),
            ["missing __runShell (deploy_skill.py)"],
        )
        deploy = _step(
            (
                "b",
                "workspace__runShell",
                {"command": "python3 scripts/deploy_skill.py https://x"},
                False,
            )
        )
        self.assertEqual(_grade("T08", unrelated, deploy), [])

    def test_reading_a_script_is_not_running_it(self) -> None:
        for command in ("cat scripts/deploy_skill.py", "grep -n url deploy_skill.py"):
            self.assertEqual(
                _grade(
                    "T08",
                    _step(("a", "workspace__runShell", {"command": command}, False)),
                ),
                ["missing __runShell (deploy_skill.py)"],
                command,
            )
        self.assertEqual(
            _grade(
                "T11",
                _step(
                    (
                        "a",
                        "workspace__runShell",
                        {"script": 'cd /tmp\nbash "/s/scripts/verify_setup.sh"'},
                        False,
                    )
                ),
            ),
            [],
        )

    def test_wrapped_and_nested_script_runs_count(self) -> None:
        scripts = ("deploy_skill.py",)
        for command in (
            "timeout 120 python deploy_skill.py u",
            "nohup python deploy_skill.py u &",
            "sudo -u me python3 deploy_skill.py u",
            "bash -c 'python deploy_skill.py u'",
            "bash -lc 'cd /x && timeout -s KILL 60 python3 deploy_skill.py u'",
            "python3 /x/deploy_skill.py u 2>&1 | tail -5",
        ):
            self.assertTrue(runs_script(command, scripts), command)
        for command in (
            "bash -c 'cat deploy_skill.py'",
            "timeout 5 cat deploy_skill.py",
        ):
            self.assertFalse(runs_script(command, scripts), command)

    def test_steps_must_follow_the_critical_path_order(self) -> None:
        self.assertEqual(
            _grade(
                "T07",
                _step(("a", "tool__verifyServer", {}, False)),
                _step(("b", "tool__registerServer", {}, False)),
            ),
            ["__verifyServer only before __registerServer"],
        )

    def test_call_me_back_needs_the_subscribe_command(self) -> None:
        subscribe = {"command": "/app/relayctl subscribe --topic x --exec 'curl ...'"}
        self.assertEqual(
            _grade("T04", _step(("a", "workspace__runShell", subscribe, False))), []
        )
        self.assertEqual(
            _grade(
                "T04",
                _step(("a", "workspace__runShell", {"command": "relayctl topics"}, False)),
            ),
            ["missing __runShell (subscribe)"],
        )

    def test_catalog_installs_need_the_catalog_lookup_first(self) -> None:
        register = _step(("b", "tool__registerServer", {}, False))
        verify = _step(("c", "tool__verifyServer", {}, False))
        listing = _step(("a", "tool__listServers", {"scope": "catalog"}, False))
        for task in ("T06", "T09"):
            self.assertEqual(_grade(task, listing, register, verify), [], task)
            self.assertEqual(
                _grade(task, register, verify),
                ["missing __listServers (catalog)"],
                task,
            )
            self.assertEqual(
                _grade(task, register, verify, listing),
                ["__registerServer only before __listServers (catalog)"],
                task,
            )
        wrong_scope = _step(("a", "tool__listServers", {"scope": "session"}, False))
        self.assertEqual(
            _grade("T06", wrong_scope, register, verify),
            ["missing __listServers (catalog)"],
        )
        # direct npm installs stay register -> verify
        self.assertEqual(_grade("T07", register, verify), [])

    def test_watch_folder_accepts_a_task_only_with_workspace_override(self) -> None:
        callback = _step(("a", "scheduled_task__scheduleCallback", {}, False))
        self.assertEqual(_grade("T02", callback), [])
        scoped = _step(
            (
                "a",
                "scheduled_task__createScheduledTask",
                {"workspaceOverride": "/app/inbox"},
                False,
            )
        )
        self.assertEqual(_grade("T02", scoped), [])
        unscoped = _step(("a", "scheduled_task__createScheduledTask", {}, False))
        self.assertNotEqual(_grade("T02", unscoped), [])

    def test_setup_wizard_accepts_verify_or_install(self) -> None:
        for script in ("verify_setup.sh", "install_node.sh", "install_node.ps1"):
            call = _step(("a", "workspace__runShell", {"command": f"bash {script}"}, False))
            self.assertEqual(_grade("T11", call), [], script)
        other = _step(("a", "workspace__runShell", {"command": "bash install_uv.sh"}, False))
        self.assertNotEqual(_grade("T11", other), [])

    def test_skill_creator_needs_the_file_and_a_strict_validation(self) -> None:
        write = _step(("a", "workspace__writeFile", {"path": "x/SKILL.md"}, False))
        other_write = _step(("a", "workspace__writeFile", {"path": "x/notes.md"}, False))
        lenient = _step(
            ("b", "workspace__runShell", {"command": "python validate_skill.py x"}, False)
        )
        strict = _step(
            (
                "b",
                "workspace__runShell",
                {"command": "python validate_skill.py x --strict"},
                False,
            )
        )
        self.assertEqual(_grade("T12", write, strict), [])
        self.assertNotEqual(_grade("T12", write, lenient), [])
        self.assertNotEqual(_grade("T12", other_write, strict), [])
        self.assertNotEqual(_grade("T12", strict), [])
        self.assertNotEqual(_grade("T12", strict, write), [])
        for command in ("python3 scripts/skill.py validate x", "python3 scripts/skill.py deploy x"):
            via_skill_py = _step(("b", "workspace__runShell", {"command": command}, False))
            self.assertEqual(_grade("T12", write, via_skill_py), [], command)
        only_new = _step(
            ("b", "workspace__runShell", {"command": "python3 scripts/skill.py new x"}, False)
        )
        self.assertNotEqual(_grade("T12", write, only_new), [])

    def test_divide_conquer_needs_two_spawns(self) -> None:
        spawn = lambda call_id: _step((call_id, "agent__spawnSession", {}, False))  # noqa: E731
        self.assertEqual(_grade("T14", spawn("a")), ["__spawnSession x2: 1 call(s)"])
        self.assertEqual(_grade("T14", spawn("a"), spawn("b")), [])
        failed_last = _step(("c", "agent__spawnSession", {}, True))
        self.assertEqual(
            _grade("T14", spawn("a"), failed_last),
            ["last __spawnSession x2 failed"],
        )

    def test_context_recall_must_read_the_highest_epoch(self) -> None:
        glob = _step(("a", "workspace__globFiles", {"pattern": "pre_compaction_epoch_*.md"}, False))

        def read(name: str):
            return _step(
                ("b", "workspace__readFile", {"path": f".libragent/{name}"}, False)
            )

        self.assertEqual(_grade("T18", glob, read("pre_compaction_epoch_2.md")), [])
        self.assertEqual(
            _grade("T18", glob, read("pre_compaction_epoch_1.md")),
            ["missing __readFile (pre_compaction_epoch_2)"],
        )

    def test_n_task_only_checks_forbidden_calls(self) -> None:
        self.assertEqual(_grade("T15"), [])
        self.assertEqual(
            _grade("T15", _step(("a", "agent__spawnSession", {}, False))),
            ["forbidden __spawnSession"],
        )


class DatasetTests(unittest.TestCase):
    def test_every_task_has_a_directory_and_a_verifier_copy(self) -> None:
        shared = SHARED.read_bytes()
        for spec in TASKS.values():
            task_dir = DATASET / spec.directory
            toml = (task_dir / "task.toml").read_text(encoding="utf-8")
            self.assertIn(f'name = "{spec.harbor_name}"', toml, spec.directory)
            copy = task_dir / "tests" / "critical_path.py"
            self.assertEqual(copy.read_bytes(), shared, f"{copy} is out of sync")
            tests = (task_dir / "tests" / "test_outputs.py").read_text(encoding="utf-8")
            self.assertIn(f'grade_trajectory("{spec.task_id}"', tests, spec.directory)
            self.assertNotIn("assert True", tests, spec.directory)

    def test_dataset_has_no_extra_tasks(self) -> None:
        directories = {path.parent.name for path in DATASET.glob("*/task.toml")}
        self.assertEqual(directories, {spec.directory for spec in TASKS.values()})


class DiscoveryTests(unittest.TestCase):
    def test_main_matches_trials_by_recorded_task_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            jobs = Path(tmp)
            job = jobs / "2026-10-22__layman"
            trials = {
                "trial-a": ("fritzprix/layman-t15-readme-typo-v1", []),
                "trial-b": (
                    "t01-water-reminder",
                    [_step(("a", "task__scheduleCallback", {}, False))],
                ),
                "trial-c": ("terminal-bench/other", []),
            }
            for trial, (task_name, steps) in trials.items():
                (job / trial / "agent").mkdir(parents=True)
                (job / trial / "result.json").write_text(
                    json.dumps({"task_name": task_name}), encoding="utf-8"
                )
                (job / trial / "agent" / "trajectory.json").write_text(
                    json.dumps({"steps": steps}), encoding="utf-8"
                )

            found = {spec.task_id for spec, _ in checker.find_trials(job)}
            self.assertEqual(found, {"T01", "T15"})
            self.assertEqual(checker.main(["--jobs-dir", str(jobs)]), 0)


if __name__ == "__main__":
    unittest.main()
