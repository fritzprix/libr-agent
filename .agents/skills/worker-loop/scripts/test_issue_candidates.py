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


if __name__ == "__main__":
    unittest.main()
