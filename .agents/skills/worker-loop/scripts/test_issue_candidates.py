#!/usr/bin/env python3
"""Tests for issue_candidates exclusion rules."""
import unittest

import issue_candidates


def reason(labels, assignees=(), me="me", number=1, closing=None):
    return issue_candidates.exclusion_reason(number, set(labels), list(assignees), me, closing or {})


class ExclusionReasonTest(unittest.TestCase):
    def test_plain_issue_is_a_candidate(self):
        self.assertIsNone(reason(["bug"]))

    def test_producer_issue_without_approved_is_skipped(self):
        for labels in (["auto:producer"], ["auto:producer", "auto:security"],
                       ["auto:producer", "validated"], ["auto:producer", "rejected"]):
            self.assertIn("approved", reason(labels), labels)

    def test_reason_names_the_validation_state(self):
        self.assertIn("(validated)", reason(["auto:producer", "validated"]))
        self.assertIn("(rejected)", reason(["auto:producer", "rejected"]))
        self.assertIn("not yet validated", reason(["auto:producer"]))

    def test_validated_alone_is_not_enough_but_approved_is(self):
        self.assertIsNotNone(reason(["auto:producer", "validated"]))
        self.assertIsNone(reason(["auto:producer", "validated", "approved"]))

    def test_approved_label_alone_unlocks_a_producer_issue(self):
        self.assertIsNone(reason(["auto:producer", "approved"]))

    def test_approved_label_does_not_affect_other_issues(self):
        self.assertIsNone(reason(["approved"]))
        self.assertIn("label: wontfix", reason(["auto:producer", "approved", "wontfix"]))

    def test_existing_exclusions_still_apply_after_approval(self):
        self.assertIn("assigned to other", reason(["auto:producer", "approved"], assignees=["other"]))
        self.assertIsNone(reason(["auto:producer", "approved"], assignees=["me"]))
        self.assertIn("#7", reason(["auto:producer", "approved"], number=3, closing={3: [7]}))

    def test_issue_claimed_by_another_worker_is_excluded(self):
        r = issue_candidates.exclusion_reason(1, set(), [], "me", {}, claimed_by="worker-2")
        self.assertEqual(r, "claimed by worker-2")


class DependsOnTest(unittest.TestCase):
    def test_depends_on_line_is_parsed(self):
        self.assertEqual(issue_candidates.depends_on("- Depends on #2543.", 2544), [2543])

    def test_related_mentions_are_not_dependencies(self):
        body = "- Depends on #2543 (the shared user prompt mechanism).\n- Related: #2544 (secret input)."
        self.assertEqual(issue_candidates.depends_on(body, 2547), [2543])

    def test_negated_dependency_after_sentence_end_is_ignored(self):
        body = "- Depends on #2543 (shared user prompt). It does not depend on #2544."
        self.assertEqual(issue_candidates.depends_on(body, 2548), [2543])

    def test_multiple_dependencies_and_blocked_by(self):
        body = "**Depends on:** #1, #2\nBlocked by #3"
        self.assertEqual(issue_candidates.depends_on(body, 9), [1, 2, 3])

    def test_no_dependency(self):
        self.assertEqual(issue_candidates.depends_on("See #5 for context.", 9), [])


class OpenDependenciesTest(unittest.TestCase):
    def unresolved(self, deps, open_issues=(), open_prs=(), merged_closes=()):
        return issue_candidates.open_dependencies(deps, set(open_issues), set(open_prs), set(merged_closes))

    def test_closed_issue_or_merged_pr_is_resolved(self):
        self.assertEqual(self.unresolved([10, 11]), [])

    def test_open_issue_closed_by_merged_pr_is_resolved(self):
        self.assertEqual(self.unresolved([2543], open_issues=[2543], merged_closes=[2543]), [])

    def test_open_issue_or_open_pr_blocks(self):
        self.assertEqual(self.unresolved([1, 2, 3], open_issues=[1], open_prs=[2]), [1, 2])


if __name__ == "__main__":
    unittest.main()
