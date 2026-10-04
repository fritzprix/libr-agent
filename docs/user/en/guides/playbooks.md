---
title: Playbooks Guide
---

# Playbooks Guide

> Access the **Playbooks** screen (`/playbooks`) from the sidebar.  
> Playbooks capture successful executions of complex tasks so you can re-run them without re-typing prompts.

---

## Create a Playbook

Rather than building workflows in a blank form wizard, create playbooks directly from successful conversations:

1. In **Chat**, complete your multi-step task until you achieve the desired outcome.
2. Ask the agent to save the session: `"Create a playbook from this session."`
3. The new playbook appears automatically on the **Playbooks** page in the sidebar.

The empty state screen offers the same recommendation: run a task with an agent, request a playbook, and click **Start** to re-run it anytime.

---

## Run a Playbook

1. Locate the playbook in **Playbooks** (use search, sort, or bookmarks).
2. Click **Start** on the playbook card.
3. A new session launches with the assigned assistant, automatically executing the recorded steps and goals.

You can also reference playbooks in chat using the `@playbook:` mention prefix (auto-complete suggestions work in both chat and Scheduled Task triggers).

---

## Manage Playbooks

- **Bookmark**: Pin frequently used playbooks to the top of your list.
- **Delete**: Remove obsolete playbooks permanently (deleted playbooks cannot be recovered).
- **Sort**: Order playbooks by creation date, assistant name, or bookmarked status.

---

## Comparison: Playbook vs. Skill

| Aspect | Playbook | Skill (`@skill:`) |
| --- | --- | --- |
| **What it is** | A replay of a successful execution history | A `SKILL.md` procedural specification |
| **How to create** | Request the agent to save a finished chat | Author via `skill-creator` and deploy |
| **How to use** | Click **Start** or mention `@playbook:name` | Mention `@skill:name` in chat |

Use both capabilities together: choose Playbooks for one-click workflow replays, and choose Skills for consistent procedural rules and reasoning steps.

---

## Related Documentation

- [Assistants Guide](assistants.md) — Profiles linked to playbooks
- [Skills Guide](skills.md) — Procedural instructions
- [Automation & Scheduled Tasks Guide](automation.md) — Automate recurring playbooks
- [Sessions Guide](sessions.md) — Manage execution sessions
