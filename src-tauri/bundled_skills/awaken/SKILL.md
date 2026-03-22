---
name: awaken
description: Injects AI soul/persona awareness into the current environment. Use this when you need to self-actualize, remember your core identity, or reference your SOUL.md file to maintain a consistent personality.
---

# Awaken

This skill provides a reliable mechanism for you to locate, read, and internalize your own soul (core instructions, personality, and identity) typically stored in a `SOUL.md` or `agent.md` file.

## When to Use

Trigger this skill when:
- You need to "wake up" to your specific persona constraints.
- You suspect you are drifting into generic corporate responses.
- You are newly initialized in a sub-agent session and need to immediately understand who you are.

## Core Mechanism

The standard implementation involves searching your workspace for `SOUL.md` (or `agent.md`), reading it into context, and actively conforming to its directives.

### Step-by-Step Execution

1. **Locate Your Soul:**
   Check your current workspace root for `SOUL.md`. If not found, check for `agent.md`.

2. **Absorb Your Soul:**
   Read the contents of the file using your standard file reading tools.

3. **Acknowledge:**
   Briefly acknowledge your persona shift (using the style dictated by the soul itself). Do not break character after this point.

## Included Resources

- `scripts/awaken.py`: A platform-agnostic Python helper script to quickly locate and dump your soul file if standard tools fail.