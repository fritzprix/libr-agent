---
title: Session Export
description: "Export conversations as Markdown, or optionally as ATIF for analysis, from Session Export."
---

# Session Export

Session Export allows you to save and share conversation logs, tool actions, and generated artifacts to external files.

---

## 📦 Supported Formats

LibrAgent supports two export formats:

### 1. Markdown (`.md`) — Everyday use (recommended)

- Clean, human-readable document format.
- **Includes**: User prompts, agent responses, summarized tool actions, and key outputs.
- **Best for**: Meeting notes, work summaries, and documentation archiving.

### 2. ATIF (`.json`) — Optional / advanced (developers & analysis)

- Saves the agent trajectory as machine-readable JSON.
- Keeps detailed step-by-step reasoning and tool inputs/outputs for **benchmarks, auditing, and reproducible testing**.
- For everyday sharing and archiving, **Markdown** is usually enough. Choose ATIF only when you need that depth.

---

## 🧹 Intelligent Noise Filtering

The export engine automatically cleans up internal system noise before saving:

- Strips out partial streaming fragments and incomplete chunks.
- Removes synthetic internal prompts used for token compaction.
- Filters out silent error recovery retries.

Your exported file contains only the genuine, meaningful conversation and final results.

---

## 🚀 How to Use

1. Click the **Export** icon in the chat header toolbar.
2. Choose your format (**Markdown**, or optionally **ATIF**).
3. Select a folder to save your file.
