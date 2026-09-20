---
title: Solution Recipes
---

# Solution Recipes

Solution Recipes are pre-built workflow templates that install everything needed for a specific use case: MCP server presets, an assistant configuration, and a scheduled task—all with one click.

---

## 🚀 Quick Start

1. Click the featured card on the **Chat home screen** (**"Create Your Morning Briefing Assistant in 5 Minutes"**) or click the recipe walkthrough button in **Scheduled Tasks**.
2. Review the pre-configured MCP presets (`hn`, `yahoo-finance`).
3. Proceed with **Install** → the recipe configures the servers, creates the assistant, and sets up the schedule automatically.
4. Run the test prompt immediately or let it trigger at its scheduled time.

---

## ☕ Morning Briefing Recipe

Automated daily briefing that combines **Hacker News** tech trends with **Yahoo Finance** market data.

### What it installs

| Component          | Details                                                                                                  |
| ------------------ | -------------------------------------------------------------------------------------------------------- |
| **MCP Presets**    | `hn` (Hacker News reader) + `yahoo-finance` (real-time market data)                                      |
| **Assistant**      | **"Morning Briefing Assistant"** (KO: `모닝 브리핑 비서`) — specialized assistant with a briefing prompt |
| **Scheduled Task** | **"Daily 9:00 AM Morning Briefing"** (KO: `매일 아침 9시 모닝 브리핑`) — runs daily at 9:00 AM           |
| **Execution Mode** | `YOLO` (auto-approves standard tool calls)                                                               |

### How it works

Every morning at 9:00 AM:

1. Reads top tech stories from Hacker News via the `hn` MCP server.
2. Retrieves key financial indicators (S&P 500, NASDAQ, exchange rates) via `yahoo-finance`.
3. Synthesizes a structured Markdown briefing report.

---

## 🛠️ Customization

- **Change schedule**: Open **Scheduled Tasks** → edit the "Daily 9:00 AM Morning Briefing" task.
- **Change content**: Edit the assistant's instructions in **Assistants**.
- **Run now**: Click the test prompt in the recipe card to verify results immediately.

---

## 💡 Build Your Own Automated Workflow

Need a custom automation? You can build one yourself anytime by combining:

1. **Extensions**: Add desired MCP presets (e.g., GitHub, Filesystem)
2. **Assistants**: Create a dedicated assistant with custom instructions and tools
3. **Scheduled Tasks**: Schedule recurring executions (Cron) in YOLO or Unsafe mode
