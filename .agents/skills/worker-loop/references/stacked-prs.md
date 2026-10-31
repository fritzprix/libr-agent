# Stacked PRs

- Do not rebase a stack while the lower PR is unmerged. Being behind the base is not a reason to rebase; do it only when a rebase conflict would otherwise block the PR, or when a change the PR needs is already in the base.
- If a lower branch was rebased, move every branch stacked above it in the same tick:
  ```bash
  git rebase --onto <new base> <old base tip>
  git push --force-with-lease
  ```
  Take `<old base tip>` from the PR's own history before the base moved (e.g. `git merge-base` with the old base SHA from `git reflog` / `pr_status.py`), not from a guess.
- Before every push, `git log --oneline origin/<base>..HEAD` must list only this PR's commits. If it shows commits that belong to another PR or to the base, restack first.
