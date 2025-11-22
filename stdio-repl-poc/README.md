# STDIO REPL POC

Proof of Concept for STDIO-based persistent shell sessions as an alternative to PTY.

## Goals

- Validate state preservation (working directory, environment variables) without PTY
- Test real-world tool availability (Python, Git, Node, Cargo)
- Verify sentinel-based command completion detection
- Ensure cross-platform consistency (Windows/Unix)

## Run

```bash
cargo run
```

## Key Differences from PTY

- **No ANSI escape codes**: PowerShell `-NonInteractive` mode outputs plain text
- **No command echo**: Clean output without shell prompts
- **Separate stderr**: Better error tracking
- **Simpler code**: ~300 lines vs PTY's 1096 lines
- **Platform unified**: Same logic for Windows and Unix

## Expected Outcome

All 7 tests should pass, demonstrating that STDIO redirection provides the same state preservation benefits as PTY, with less complexity.

## Next Steps

If successful, integrate into LibrAgent as `PersistentShellManager` replacing current PTY approach.
