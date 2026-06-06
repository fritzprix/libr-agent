---
name: x-cli
description: |
  Python CLI wrapper for X (Twitter) interaction using Twikit.
  Use when the user wants to: (1) post tweets (with text or media), (2) view home timeline,
  (3) view a user's tweets, (4) search tweets, (5) like/favorite a tweet, or (6) retweet a tweet.
  On first use, guide the user through credential setup (username, email, password, optional TOTP) or browser cookie setup.
  Store session and config in ~/.libragent/x_config.json.
  Subsequent requests use the stored session without re-authentication.
  Triggers on requests like: "트윗 올려줘", "트위터 피드 보여줘", "post a tweet", "like tweet", "retweet".
---

# X (Twitter) CLI Skill

Enables the agent to interact with X (Twitter) on behalf of the user via the Twikit library.

## Path conventions

Paths in this skill are relative to the directory containing this `SKILL.md`, not to the workspace root or the shell's current `./`.

- Scripts in this skill use paths like `scripts/...`
- When a command below says `python scripts/...`, resolve that script path against the skill's absolute Base Directory.
- In command examples below, replace `<skill-base-dir>` with the skill's actual absolute Base Directory.

## ⚠️ Security Rules (Mandatory)

- **NEVER** ask for password, TOTP secrets, or browser cookies in plain chat.
- **NEVER** display or repeat the contents of `~/.libragent/x_config.json` or `~/.libragent/x_cookies.json`.
- Collect password and other sensitive information exclusively through interactive hidden shell prompts (`requireUserInput=true`, `inputType=password`).
- **ALWAYS** use `setup.py` to persist credentials and session.

---

## Overview

X integration involves these steps:

1. **Detect config** — run `check_config.py` to check if account is configured.
2. **Setup (first time only)** — configure via credentials or browser cookies (highly recommended due to Twitter API changes).
3. **Dispatch action** — call `x_cli.py` with the right action.
4. **Present results** — format and summarize the output for the user.

---

## Prerequisites

Ensure Twikit is installed:

```bash
pip install twikit
```

---

## Step 1: Detect Config

Always start by checking if the account is configured:

```bash
python "<skill-base-dir>/scripts/check_config.py"
```

- Exit code `0` → configured, proceed to Step 3.
- Exit code `1` or `2` → not configured/corrupt, proceed to Step 2.

---

## Step 2: Account Setup (First Time or Reset)

### Option A: Cookie-based Setup (Recommended & Bulletproof)

Because X frequently blocks automated credentials-based login with a `404 (code 34)` error on the guest token endpoint, using browser cookies is the most reliable way to authenticate.

1. Log in to [x.com](https://x.com) in your web browser.
2. Open Developer Tools (F12) -> **Application** (or **Storage**) -> **Cookies** -> `https://x.com`.
3. Copy the values of `auth_token` and `ct0`.
4. Run the terminal cookie wizard:
   ```bash
   python "<skill-base-dir>/scripts/setup.py" \
     --username "your_username" \
     --email "your_email@domain.com" \
     --cookie-login
   ```
5. When prompted in the terminal, paste `auth_token` and `ct0`. The inputs are hidden and are not echoed back.

If the user already has the cookie values ready and explicitly prefers a one-shot command, `--auth-token` and `--ct0` still work, but the wizard is the default recommendation because it keeps secrets out of shell history.

### Option B: Credentials-based Setup (Fallback)

Execute `runInPersistentShell` (or `runInPersistentPowerShell`) with:

- `requireUserInput=true`
- `inputType=password`
- `inputPrompt=X 비밀번호를 입력하세요:`
- Command (replace username and email with user-supplied ones):
  ```bash
  python "<skill-base-dir>/scripts/setup.py" \
    --username "your_username" \
    --email "your_email@domain.com" \
    --password-stdin
  ```

---

## Step 3: Dispatch Action

Call `x_cli.py` to execute X actions:

If credentials login returns `404` with `code 34`, stop retrying passwords and switch to Option A immediately. Always give the user the concrete next step: open browser cookies, then run the cookie wizard command.

### Action: `post_tweet` — Post a new tweet

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action post_tweet \
  --message "트윗 내용" \
  [--file "/path/to/attachment"]
```

### Action: `get_timeline` — View home timeline

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action get_timeline \
  [--limit 10]
```

### Action: `get_user_tweets` — View specific user's tweets

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action get_user_tweets \
  --username "target_username" \
  [--limit 10]
```

### Action: `search_tweets` — Search tweets by query

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action search_tweets \
  --query "검색어" \
  [--limit 10]
```

### Action: `like_tweet` — Like/favorite a tweet

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action like_tweet \
  --tweet-id "1234567890"
```

### Action: `retweet` — Retweet a tweet

```bash
python "<skill-base-dir>/scripts/x_cli.py" --action retweet \
  --tweet-id "1234567890"
```
