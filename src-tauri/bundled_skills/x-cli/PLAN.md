# PLAN: X (Twitter) Skill (x-cli)

This document outlines the planning, requirements, and design for the X integration skill (`x-cli`).

## Purpose
Enable agents to interact with X (Twitter) on behalf of the user to post tweets (with/without media), read timelines, search tweets, and perform interactions like liking and retweeting.

## Technology Stack
- **Python 3**: For CLI scripts.
- **Twikit (>=2.0.0)**: A library using Twitter's private/internal API with an async client API, wrapped by `asyncio.run()` in the CLI scripts.
- **JSON Configuration**: Config and session cookies stored under `~/.libragent/`.

## File Structure
We will create the skill in two locations:
1. `.agents/skills/x-cli/` (active agent workspace)
2. `src-tauri/bundled_skills/x-cli/` (packaged application assets)

Files to create in each directory:
- `SKILL.md`: Skill descriptor and onboarding instructions for the LLM.
- `requirements.txt`: Python package requirements (`twikit`).
- `scripts/check_config.py`: Status validation.
- `scripts/setup.py`: Session setup / login credentials.
- `scripts/x_cli.py`: Actions dispatcher.

## Security Considerations
- Username, email, and password will be parsed through hidden stdin prompts during interactive setup.
- Session cookies will be saved in `~/.libragent/x_cookies.json` to prevent repeatedly logging in and triggering security flags or account locks.
- Config is kept separate and private in `~/.libragent/x_config.json`.
