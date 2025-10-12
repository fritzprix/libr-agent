# Git Configuration for Line Endings

## TL;DR

Run this command to improve line ending handling on Windows:

```bash
git config core.autocrlf input
```

## Explanation

### Current Setup (After `.gitattributes`)

- **Repository**: All text files stored with LF (Unix-style)
- **Your Working Directory**: Files can use CRLF on Windows (for compatibility)
- **.gitattributes**: Enforces LF in repository, regardless of local OS

### Recommended Git Config

#### For This Project Only

```bash
git config core.autocrlf input
```

#### For All Your Projects (Global)

```bash
git config --global core.autocrlf input
```

### What This Does

- **On Commit**: Converts CRLF → LF (if you accidentally create files with CRLF)
- **On Checkout**: Leaves files as-is (LF stays LF)
- **Result**: Repository always has LF, working directory can have either

### Why This Is Better Than `autocrlf = true`

- `true`: Git converts LF → CRLF on checkout, CRLF → LF on commit
- `input`: Git only converts on commit, doesn't touch files on checkout
- With `.gitattributes` in place, `input` is safer and more predictable

### Verification

After changing the config:

```bash
# Check current setting
git config core.autocrlf

# Pull latest changes
git pull

# Verify no files show as modified
git status
```

### For Linux/macOS Users

```bash
git config core.autocrlf input
```

(Same setting works for all platforms)

## Troubleshooting

### If You Still See Line Ending Warnings

```bash
# Reset Git's line ending cache
git rm --cached -r .
git reset --hard
```

### If Files Show as Modified After Switching Platforms

This should NOT happen after our changes. If it does:

1. Check `.gitattributes` is committed
2. Verify `git config core.autocrlf` is set to `input`
3. Report the issue - it means we missed something

## References

- [Git Documentation - core.autocrlf](https://git-scm.com/docs/git-config#Documentation/git-config.txt-coreautocrlf)
- [gitattributes Documentation](https://git-scm.com/docs/gitattributes)
