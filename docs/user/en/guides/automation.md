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

## Other automation (not this page)

| Mechanism              | When                                                              |
| ---------------------- | ----------------------------------------------------------------- |
| **Scheduled Tasks**    | App-wide / recurring / cron background runs                       |
| **`@skill:loop`**      | **In this session** clock-based loops / reminders / delays        |
| **`@skill:call-me-back`** | Resume on process / kanban / webhook **completion signals**    |
| **`@skill:schedule`**  | Agent-facing schedule operating procedures                        |
| **Org / teamwork**     | Explicit team lineage — sidebar **Org**                           |

## Tips

- Prefer low-risk tools for unattended runs.
- Confirm API keys and network before enabling.
- Check History after the first few runs.
- Optional **workspace override** on a task targets a folder; SESSION callbacks do not clear a pinned chat session’s own override when the task leaves that field empty.

Failures often look like normal session errors — see [Troubleshooting](troubleshooting.md).

---

## Related

- [Playbooks](playbooks.md) · [Sessions](sessions.md)
