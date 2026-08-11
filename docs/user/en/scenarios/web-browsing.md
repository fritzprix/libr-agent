---
title: Scenario - Autonomous Web Browsing
---

# Scenario: Autonomous Web Browsing

Enable AI agents to navigate web pages, inspect DOM elements, trigger button clicks, fill forms, and extract live web data.

---

## 🎯 Objectives

- Automate web information gathering across dynamic JavaScript web apps.
- Extract tabular data, search results, or live API docs directly from the web.
- Interact with web UI forms using `navigateToUrl`, `clickElement`, and `inputText`.

---

## 📋 Step-by-Step Guide

### Step 1: Provide Navigation Goal

```
Navigate to https://news.ycombinator.com, extract the top 5 articles with their vote counts and post URLs.
```

### Step 2: Agent Execution & Inspection

The agent uses headless browser tools (`navigateToUrl`, `listInteractable`, `evaluateJS`) to navigate pages and extract DOM content safely.

### Step 3: Review Extracted Web Data

```markdown
## 📰 Top Hacker News Stories

1. **Title A** - 450 points - [Link](https://...)
2. **Title B** - 320 points - [Link](https://...)
```
