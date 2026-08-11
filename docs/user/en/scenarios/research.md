---
title: Scenario - Deep Research & Reporting
---

# Scenario: Deep Research & Reporting

Use LibrAgent to perform comprehensive technical research, summarize online documentation, query arXiv/biomedical databases, and synthesize executive reports.

---

## 🎯 Objectives

- Perform multi-query deep web and literature research.
- Extract key findings, benchmarks, and comparison matrices.
- Export findings into clean Markdown reports.

---

## 📋 Step-by-Step Guide

### Step 1: Define Research Topic

Ask the agent to research a specific framework, paper, or technology:

```
Research the latest web browser automation MCP servers and compare their performance, API capabilities, and license terms.
```

### Step 2: Interactive Clarification

If necessary, the agent may use `ask_question` to clarify scope, target sources, or output formats.

### Step 3: View Research Synthesis Report

The agent executes web browsing, paper lookup, or code inspection tools and generates a formatted report:

```markdown
## 📊 Research Summary: Browser Automation MCP Tools

| Tool           | Engine          | SSE Support | License    |
| :------------- | :-------------- | :---------- | :--------- |
| Puppeteer MCP  | Chrome Headless | Yes         | MIT        |
| Playwright MCP | Multi-browser   | Yes         | Apache-2.0 |
```
