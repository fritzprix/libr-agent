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

from layman_intent_critical_path import TASKS, grade_calls, load_calls


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
                "T06",
                _step(("a", "tool__registerServer", {}, True)),
                _step(("b", "tool__registerServer", {}, False)),
                _step(("c", "tool__verifyServer", {}, False)),
            ),
            [],
        )

    def test_missing_step_and_failed_last_call(self) -> None:
        failures = _grade(
            "T06",
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
