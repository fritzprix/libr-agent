#!/usr/bin/env python3
"""Tests for issue_candidates: the `approved` gate and closing-PR detection."""
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
