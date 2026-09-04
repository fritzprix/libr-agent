# Proposal checklist

## Before editing

- [ ] Global wiki initialized (`wiki_cli.py init` / `path`)
- [ ] Pattern id exists under the host wiki `patterns/`
- [ ] `skill-impact.md` checked for a prior Reject of the same idea
- [ ] Single target skill identified
- [ ] User-facing mechanism written in one sentence

## Patch quality

- [ ] One file (or one coherent skill folder change) only
- [ ] Frontmatter still has `name` and a `description` with trigger phrasing (“use when” / “when the user” / …)
- [ ] No “When to use” section duplicated in the Markdown body
- [ ] Relative `references/` links resolve inside the skill package
- [ ] No checkout-only paths; no evaluation-task or grader instructions
- [ ] Works as guidance on Windows, macOS, and Linux (no OS-only path assumptions except documenting home wiki via `wiki_cli path`)

## After editing

- [ ] Self-review or compliance audit
- [ ] Ledger row appended via `wiki_cli.py append-impact`
- [ ] On Reject, skill content restored; wiki kept
