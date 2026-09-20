---
title: Session Export
---

# Session Export

Session Export allows you to save and share conversation logs, tool actions, and generated artifacts to external files.

---

## 📦 Supported Formats

LibrAgent supports two export formats:

### 1. Markdown (`.md`) — For Reports & Sharing

- Clean, human-readable document format.
- **Includes**: User prompts, agent responses, summarized tool actions, and key outputs.
- **Best for**: Meeting notes, work summaries, and documentation archiving.

### 2. ATIF v1.7 (`.json`) — For Trajectory Analysis & Benchmarking

- **Agent Trajectory Interchange Format** (standard benchmark trajectory).
- Captures granular chain-of-thought steps, detailed tool inputs/outputs, and state transitions.
- **Best for**: Benchmark evaluations, performance auditing, and reproducible testing.

---

## 🧹 Intelligent Noise Filtering

The export engine automatically cleans up internal system noise before saving:

- Strips out partial streaming fragments and incomplete chunks.
- Removes synthetic internal prompts used for token compaction.
- Filters out silent error recovery retries.

Your exported file contains only the genuine, meaningful **agent trajectory** and final results.

---

## 🚀 How to Use

1. Click the **Export** icon in the chat header toolbar.
2. Choose your format (**Markdown** or **ATIF JSON**).
3. Select a folder to save your file.
