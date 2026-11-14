#!/usr/bin/env python3
"""Tests for issue_candidates: the `approved` gate, closing-PR detection,
`needs-owner` answers, and open dependencies."""
import json
import unittest

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


def fake_gh(comments=(), states=None, blocked_by=None):
    """gh stand-in: issue comments, issue states by number, and the blocked-by API."""
    def _gh(*args):
        if args[:2] == ("issue", "view"):
            return json.dumps({"comments": list(comments)})
        if args[0] == "api" and args[1].endswith("/dependencies/blocked_by"):
            if blocked_by is None:
                raise issue_candidates.GhError("404")
            return json.dumps(blocked_by)
        if args[0] == "api":
            return (states or {}).get(int(args[1].rsplit("/", 1)[1]), "closed") + "\n"
        raise ValueError(f"Unexpected gh call: {args}")
    return _gh


def waiting(labels=("approved",), body="", **gh_kwargs):
    issue = {"number": 5, "body": body}
    return issue_candidates.waiting_reason(issue, set(labels), "me", fake_gh(**gh_kwargs))


class DependencyRefsTest(unittest.TestCase):
    def test_finds_depends_on_and_blocked_by(self):
        body = "Part of #1. Depends on #2749. Continues #2704.\nBlocked by #3, #4, and #5"
        self.assertEqual(issue_candidates.dependency_refs(body), [2749, 3, 4, 5])

    def test_plain_mentions_are_not_dependencies(self):
        self.assertEqual(issue_candidates.dependency_refs("Part of #1. See #2."), [])
        self.assertEqual(issue_candidates.dependency_refs(None), [])


class WaitingReasonTest(unittest.TestCase):
    def test_free_issue_does_not_wait(self):
        self.assertEqual(waiting(), (None, None))

    def test_open_dependency_blocks(self):
        reason, _ = waiting(body="Depends on #7 and #8", states={7: "open", 8: "closed"})
        self.assertEqual(reason, "blocked by open #7")
        self.assertEqual(waiting(body="Depends on #7", states={7: "closed"}), (None, None))

    def test_github_blocked_by_counts_when_available(self):
        self.assertEqual(waiting(blocked_by=[9])[0], "blocked by open #9")
        self.assertEqual(waiting(body="Blocked by #9", states={9: "open"}, blocked_by=[9])[0],
                         "blocked by open #9")

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

    def test_answered_issue_still_waits_on_open_dependency(self):
        answer = comment("Go with (a).", "2026-01-02T00:00:00Z")
        reason, found = waiting(labels=("approved", "needs-owner"), comments=[QUESTION, answer],
                                body="Depends on #7", states={7: "open"})
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


if __name__ == "__main__":
    unittest.main()
