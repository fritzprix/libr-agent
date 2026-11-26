#!/usr/bin/env python3
"""Tests for issue_deps, the shared GitHub dependency helper (issue #2995).

Run: python3 .agents/skills/loop-shared/scripts/test_issue_deps.py
"""
import json
import unittest

import issue_deps
from issue_deps import DepsError

NOT_FOUND = "gh api failed: gh: Not Found (HTTP 404)"


def fake_gh(blockers=(), states=None, blocker_id=999, fail_post=False, missing=(),
            fail_read=False):
    """gh stand-in for the dependency API.

    - `blockers`: current blocked-by list entries (`number`/`state` dicts).
    - `states`: `GET issues/N` responses by number; numbers in `missing` (or
      not in `states`) are 404.
    - `fail_read`/`fail_post`: raise a non-404 error on that call kind.
    """

    def number_of(path):
        return int(path.split("/issues/", 1)[1].split("/", 1)[0])

    def _gh(*args):
        if args[:2] == ("api", "--method"):
            if number_of(args[3]) in missing:
                raise DepsError(NOT_FOUND)
            if fail_post:
                raise DepsError("gh api failed: API rate limit exceeded (HTTP 403)")
            return ""
        if args[0] == "api" and args[1].endswith("/dependencies/blocked_by"):
            if number_of(args[1]) in missing:
                raise DepsError(NOT_FOUND)
            if fail_read:
                raise DepsError("gh api failed: API rate limit exceeded (HTTP 403)")
            return json.dumps(list(blockers))
        if args[0] == "api" and args[1].startswith("repos/{owner}/{repo}/issues/"):
            number = int(args[1].rsplit("/", 1)[1])
            state = (states or {}).get(number)
            if state is None or state == "missing":
                raise DepsError(NOT_FOUND)
            return json.dumps({"number": number, "state": state, "id": blocker_id})
        raise ValueError(f"Unexpected gh call: {args}")
    return _gh


class BlockedByTest(unittest.TestCase):
    def test_returns_numbers_and_states(self):
        found = issue_deps.blocked_by(5, fake_gh(blockers=[{"number": 9, "state": "open"},
                                                           {"number": 2, "state": "closed"}]))
        self.assertEqual(found, [{"number": 9, "state": "open"}, {"number": 2, "state": "closed"}])

    def test_empty_list_when_no_blockers(self):
        self.assertEqual(issue_deps.blocked_by(5, fake_gh()), [])

    def test_404_is_an_error_not_an_empty_list(self):
        with self.assertRaises(DepsError):
            issue_deps.blocked_by(404, fake_gh(missing=(404,)))

    def test_other_errors_are_raised(self):
        with self.assertRaises(DepsError):
            issue_deps.blocked_by(5, fake_gh(fail_read=True))


class OpenBlockersTest(unittest.TestCase):
    def test_only_open_blockers_are_returned(self):
        found = issue_deps.open_blockers(5, fake_gh(blockers=[{"number": 9, "state": "open"},
                                                              {"number": 2, "state": "closed"},
                                                              {"number": 7, "state": "open"}]))
        self.assertEqual(found, [9, 7])

    def test_404_is_an_error(self):
        with self.assertRaises(DepsError):
            issue_deps.open_blockers(404, fake_gh(missing=(404,)))


class AddBlockerTest(unittest.TestCase):
    def test_post_uses_integer_id_not_number(self):
        calls = []

        def gh(*args):
            if args[:2] == ("api", "--method"):
                calls.append(args)
                return ""
            return fake_gh(states={7: "open"}, blocker_id=12345)(*args)

        issue_deps.add_blocker(5, 7, gh)
        self.assertEqual(len(calls), 1)
        self.assertIn("issue_id=12345", calls[0])

    def test_second_call_is_idempotent(self):
        calls = []

        def gh(*args):
            if args[:2] == ("api", "--method"):
                calls.append(args)
                return ""
            return fake_gh(blockers=[{"number": 7, "state": "open"}], states={7: "open"})(*args)

        issue_deps.add_blocker(5, 7, gh)
        issue_deps.add_blocker(5, 7, gh)
        self.assertEqual(calls, [])

    def test_missing_blocker_is_an_error(self):
        with self.assertRaises(DepsError):
            issue_deps.add_blocker(5, 404, fake_gh(states={}))

    def test_missing_target_is_an_error_on_post_404(self):
        with self.assertRaises(DepsError):
            issue_deps.add_blocker(404, 7, fake_gh(states={7: "open"}, missing=(404,)))

    def test_post_failure_other_than_404_is_raised(self):
        with self.assertRaises(DepsError):
            issue_deps.add_blocker(5, 7, fake_gh(states={7: "open"}, fail_post=True))

    def test_blocker_without_integer_id_is_an_error(self):
        def gh(*args):
            if args[:2] == ("api", "--method"):
                return ""
            if args[1].endswith("/dependencies/blocked_by"):
                return "[]"
            return json.dumps({"number": 7, "state": "open", "id": "seven"})

        with self.assertRaises(DepsError):
            issue_deps.add_blocker(5, 7, gh)


class PrBlockersTest(unittest.TestCase):
    """Owner decision (a): PR blockers live on the PR's closing issue."""

    def test_pr_blockers_reads_closing_issue_not_pr_number(self):
        seen_paths = []

        def gh(*args):
            seen_paths.append(args[1])
            return json.dumps([{"number": 99, "state": "open"}])

        pr = {"number": 3026, "title": "Fix thing", "body": "Fixes #2995", "headRefName": "fix/thing-2995"}
        self.assertEqual(issue_deps.pr_open_blockers(pr, gh), [99])
        self.assertEqual(seen_paths, ["repos/{owner}/{repo}/issues/2995/dependencies/blocked_by"])

    def test_pr_without_closing_issue_returns_empty_without_calling_gh(self):
        called = []
        pr = {"number": 3026, "title": "Chore", "body": "", "headRefName": "chore/bump-deps-2026"}
        self.assertEqual(issue_deps.pr_blockers(pr, lambda *a: called.append(a) or "[]"), [])
        self.assertEqual(called, [])

    def test_blocked_by_preserves_closed_at_when_present(self):
        gh = fake_gh(blockers=[{"number": 9, "state": "closed", "closed_at": "2026-11-25T15:00:00Z"}])
        self.assertEqual(
            issue_deps.blocked_by(5, gh),
            [{"number": 9, "state": "closed", "closed_at": "2026-11-25T15:00:00Z"}],
        )


if __name__ == "__main__":
    unittest.main()
