---
title: Automation
---

# Automation

> Run agents on a schedule with **Scheduled Tasks**.

---

## Open Scheduled Tasks

Sidebar → **Scheduled Tasks** (or Automation entry, depending on version).

Create a task: choose assistant / playbook, schedule (cron or interval), and enable it. You can also pick from built-in **Starter Templates** (such as Daily Standup or Code Review Digest) for instant 1-click configuration.

---

## Typical fields

| Field            | Meaning      |
| ---------------- | ------------ |
| Name             | Task label   |
| Schedule         | When it runs |
| Agent / Playbook | What to run  |
| Enabled          | On / off     |

---

## Tips

- Prefer low-risk tools for unattended runs.
- Confirm API keys and network before enabling.
- Check History after the first few runs.

Failures often look like normal session errors — see [Troubleshooting](troubleshooting.md).

---

## Related

- [Playbooks](playbooks.md) · [Sessions](sessions.md)
