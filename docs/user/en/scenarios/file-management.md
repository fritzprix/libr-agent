---
title: Scenario - File Management & Batch Processing
---

# Scenario: File Management & Batch Processing

Automate document summarization, file reorganization, formatting, and text extraction using built-in workspace and attachment tools.

---

## 🎯 Objectives

- Batch process documents, logs, or dataset files using natural language.
- Extract structured tables or summaries from uploaded session attachments.
- Refactor file structures safely within the workspace.

---

## 📋 Step-by-Step Guide

### Step 1: Specify Target Files

Instruct the agent using workspace relative paths or session file attachments:

```
Read the uploaded PDF report attachment and summarize key financial highlights.
```

_(The agent utilizes `attachments__readAttachment` and `workspace__readFile` to process the files)_

### Step 2: Request File Operations

```
Convert the sales data in `./data/sales-2026.csv` into a formatted Markdown comparison table.
```

### Step 3: Verify Output

The agent automatically edits or creates target files using `workspace__writeFile` / `workspace__strReplace`.
