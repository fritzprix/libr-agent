# Error Handling

| Error | Cause | Action |
| --- | --- | --- |
| Not a git repository | Workspace not on a repo | `workspace` connect correct folder |
| `gh auth` failed | Not logged in | `gh auth login` |
| Nothing to commit | Clean tree | Skip commit; push/PR only |
| `pr_create` failed | No commits ahead of base | Push branch first |
| Checks failing | CI red | Report failing checks; do not merge unless user overrides |
| Merge conflict | Branch behind base | Ask user: rebase/merge base locally |
| No tag for log_since_tag | First release | Use `HEAD~N` or full history |

Always return the **next command** the user or agent should run.
