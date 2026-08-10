---
title: Scenario - Scheduled Tasks & Automation Workflows
---

# Scenario: Scheduled Tasks & Automation Workflows

Configure recurring background timers, Cron notifications, and autonomous multi-agent task execution.

---

## 🎯 Objectives

- Set up background reminders and periodic health/status checks.
- Automate multi-step background workflows without blocking active chat sessions.
- Manage active timers via `scheduled_task__*` built-in tools.

---

## 📋 Step-by-Step Guide

### Step 1: Request a Scheduled Task

```
Set a recurring Cron task to check server health every 30 minutes.
```

### Step 2: Manage Active Schedules

List or remove active scheduled tasks:

- View active tasks: `scheduled_task__list`
- Delete schedule: `scheduled_task__delete`
