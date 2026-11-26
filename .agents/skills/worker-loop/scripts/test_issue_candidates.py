#!/usr/bin/env python3
"""Tests for issue_candidates: the `approved` gate, closing-PR detection,
`needs-owner` answers, and open dependencies."""
import json
import sys
import unittest
from pathlib import Path

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

from issue_deps import DepsError

import issue_candidates


def reason(labels, assignees=(), me="me", number=1, closing=None):
    return issue_candidates.exclusion_reason(number, set(labels), list(assignees), me, closing or {})


def pr(number=1, title="", body="", head="fix/some-slug"):
    return {"number": number, "title": title, "body": body, "headRefName": head}


class ExclusionReasonTest(unittest.TestCase):
    def test_unapproved_issue_is_never_a_candidate(self):
        for labels in (["bug"], ["auto:producer"], ["auto:producer", "validated"], ["enhancement"]):
            self.assertEqual(reason(labels), "no 'approved' label", labels)

    def test_approved_label_is_enough(self):
        self.assertIsNone(reason(["approved"]))
        self.assertIsNone(reason(["bug", "approved"]))
        self.assertIsNone(reason(["auto:producer", "validated", "approved"]))

    def test_validated_alone_is_not_approval(self):
        self.assertIsNotNone(reason(["auto:producer", "validated"]))

    def test_closed_labels_still_apply_after_approval(self):
        for label in sorted(issue_candidates.EXCLUDED_LABELS):
            self.assertIn(label, reason(["approved", label]))

    def test_epic_and_auto_e2e_are_not_candidates(self):
        self.assertIn("epic", reason(["approved", "epic"]))
        self.assertIn("auto:e2e", reason(["approved", "auto:e2e"]))

    def test_rejected_is_not_a_candidate_even_with_approved(self):
        self.assertIn("rejected", reason(["approved", "rejected"]))

    def test_assignment_rules(self):
        self.assertIn("assigned to other", reason(["approved"], assignees=["other"]))
        self.assertIsNone(reason(["approved"], assignees=["me"]))
        self.assertIsNone(reason(["approved"], assignees=[]))

    def test_issue_covered_by_open_pr_is_excluded(self):
        found = reason(["approved"], number=3, closing={3: [7, 8]})
        self.assertIn("#7", found)
        self.assertIn("#8", found)
        self.assertIsNone(reason(["approved"], number=3, closing={4: [7]}))

    def test_issue_claimed_by_another_worker_is_excluded(self):
        found = issue_candidates.exclusion_reason(1, {"approved"}, [], "me", {}, claimed_by="worker-2")
        self.assertEqual(found, "claimed by worker-2")


def comment(body, at, login="me", association="NONE"):
    return {"author": {"login": login}, "authorAssociation": association, "body": body,
            "createdAt": at, "url": f"u-{at}"}


QUESTION = comment("<!-- worker-loop: needs-owner -->\nWhich one? (a) or (b)", "2026-01-01T00:00:00Z")


NOT_FOUND = "gh api failed: gh: Not Found (HTTP 404)"


def fake_gh(comments=(), blocked_by=(), blocked_by_error=None):
    """gh stand-in: issue comments and the blocked-by API.

    `blocked_by` entries are `{"number", "state"}` dicts as the dependency API
    returns them; `blocked_by_error` raises (default: no error, empty list).
    """
    def _gh(*args):
        if args[:2] == ("issue", "view"):
            return json.dumps({"comments": list(comments)})
        if args[0] == "api" and args[1].endswith("/dependencies/blocked_by"):
            if blocked_by_error:
                # Mirrors the helper: its own gh raises DepsError on failure.
                raise DepsError(blocked_by_error)
            return json.dumps(list(blocked_by))
        raise ValueError(f"Unexpected gh call: {args}")
    return _gh


def waiting(labels=("approved",), body="", **gh_kwargs):
    issue = {"number": 5, "body": body}
    return issue_candidates.waiting_reason(issue, set(labels), "me", fake_gh(**gh_kwargs))


class WaitingReasonTest(unittest.TestCase):
    def test_free_issue_does_not_wait(self):
        self.assertEqual(waiting(), (None, None))

    def test_open_api_blocker_blocks_and_closed_does_not(self):
        api_blockers = [{"number": 9, "state": "open"}, {"number": 8, "state": "closed"}]
        self.assertEqual(waiting(blocked_by=api_blockers)[0], "blocked by open #9")
        self.assertEqual(waiting(blocked_by=[{"number": 9, "state": "closed"}]), (None, None))

    def test_body_text_dependency_lines_are_ignored(self):
        self.assertEqual(waiting(body="Depends on #7 and #8"), (None, None))
        self.assertEqual(waiting(body="Blocked by #9"), (None, None))

    def test_blocked_by_failure_is_raised(self):
        with self.assertRaises(issue_candidates.GhError):
            waiting(blocked_by_error="gh api failed: API rate limit exceeded (HTTP 403)")

    def test_unanswered_needs_owner_waits(self):
        reason, answer = waiting(labels=("approved", "needs-owner"), comments=[QUESTION])
        self.assertIn("needs-owner", reason)
        self.assertIsNone(answer)

    def test_loop_comments_after_the_question_are_not_answers(self):
        later = comment("<!-- gatekeeper-loop: issue-validation -->\nok", "2026-01-02T00:00:00Z")
        self.assertIsNotNone(waiting(labels=("approved", "needs-owner"), comments=[QUESTION, later])[0])

    def test_owner_answer_resumes_the_issue(self):
        answer = comment("Go with (a).", "2026-01-02T00:00:00Z")
        reason, found = waiting(labels=("approved", "needs-owner"), comments=[QUESTION, answer])
        self.assertIsNone(reason)
        self.assertEqual(found["url"], answer["url"])

    def test_outsider_comment_is_not_an_answer(self):
        outsider = comment("Do (b)!", "2026-01-02T00:00:00Z", login="stranger")
        member = comment("Do (a).", "2026-01-03T00:00:00Z", login="maint", association="MEMBER")
        self.assertIsNotNone(waiting(labels=("approved", "needs-owner"), comments=[QUESTION, outsider])[0])
        self.assertIsNone(waiting(labels=("approved", "needs-owner"), comments=[QUESTION, outsider, member])[0])

    def test_answered_issue_still_waits_on_open_blocker(self):
        answer = comment("Go with (a).", "2026-01-02T00:00:00Z")
        reason, found = waiting(labels=("approved", "needs-owner"), comments=[QUESTION, answer],
                                blocked_by=[{"number": 7, "state": "open"}])
        self.assertEqual(reason, "blocked by open #7")
        self.assertIsNotNone(found)


class ClosingRefsTest(unittest.TestCase):
    def test_closing_keyword_in_title(self):
        self.assertEqual(issue_candidates.closing_refs(pr(title="Fix #12 crash")), {12})

    def test_closing_keywords_in_body(self):
        body = "fixes #12\nFixed #13\nresolves #14\nclosed #15\nCloses #16"
        self.assertEqual(issue_candidates.closing_refs(pr(body=body)), {12, 13, 14, 15, 16})

    def test_plain_mention_is_not_a_close(self):
        self.assertEqual(issue_candidates.closing_refs(pr(body="Related to #12, see #13.")), set())

    def test_branch_number_counts_as_a_close(self):
        self.assertEqual(issue_candidates.closing_refs(pr(head="fix/keychain-fallback-stale-2588")), {2588})

    def test_missing_body_is_safe(self):
        self.assertEqual(issue_candidates.closing_refs(pr(title="Fixes #9", body=None)), {9})


class ClosingPrMapTest(unittest.TestCase):
    def test_maps_each_issue_to_its_closing_prs(self):
        prs = [pr(7, title="Fixes #12"), pr(8, body="Closes #12 and fixes #13")]
        self.assertEqual(issue_candidates.closing_pr_map(prs), {12: [7, 8], 13: [8]})

    def test_no_prs_maps_to_nothing(self):
        self.assertEqual(issue_candidates.closing_pr_map([]), {})


class ParentIssueExclusionTest(unittest.TestCase):
    """Issue #3030: parent issues with sub-issues are excluded even without `epic` label."""

    def test_parent_with_open_sub_issues_is_excluded_naming_sub_issues(self):
        reason = issue_candidates.exclusion_reason(
            3022, {"approved"}, [], "me", {},
            run_gh=lambda *a: "[3010, 3011]",
        )
        self.assertIsNotNone(reason)
        self.assertIn("#3010", reason)
        self.assertIn("#3011", reason)

    def test_parent_with_all_closed_sub_issues_is_still_excluded(self):
        # The sub-issues endpoint returns both open and closed children; any child excludes the parent.
        reason = issue_candidates.exclusion_reason(
            2900, {"approved"}, [], "me", {},
            run_gh=lambda *a: "[2901, 2902]",
        )
        self.assertIsNotNone(reason)
        self.assertIn("#2901", reason)
        self.assertIn("#2902", reason)

    def test_epic_label_without_sub_issues_is_still_excluded(self):
        called = []
        reason = issue_candidates.exclusion_reason(
            2920, {"approved", "epic"}, [], "me", {},
            run_gh=lambda *a: called.append(a) or "[]",
        )
        self.assertEqual(reason, "label: epic")
        self.assertEqual(called, [])


if __name__ == "__main__":
    unittest.main()
