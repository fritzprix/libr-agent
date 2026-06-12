---
name: data-viz
description: |
  End-user data visualization — turn CSV, Excel, or JSON into standalone HTML charts and small dashboards.
  Use when the user wants charts, trends, or visual reports from tabular data
  ("chart this CSV", "show monthly sales trend", "make a dashboard from this spreadsheet").
  Not for document editing (docx/pptx) or raw format conversion (to-md).
  Triggers on: "차트 만들어줘", "데이터 시각화", "plot this data", "dashboard from CSV".
---

# Data Visualization

Turn **the user's tabular data** into interactive HTML charts they can open in a browser.

End-user **analysis and reporting** skill — not LibrAgent internal analytics.

## Not This Skill

| Skill | Use for |
| --- | --- |
| **to-md** | Extract text from PDF/XLSX without charting |
| **docx** / **pptx** | Author formatted documents |
| **deep-research** | Narrative research reports |
| **workspace-indexer** | Batch markdown index for a repo |

## Path conventions

Replace `<skill-base-dir>` with this skill's absolute Base Directory.

## Prerequisites

CSV and JSON work with the Python standard library.

For Excel (`.xlsx`):

```bash
pip install pandas openpyxl
```

Charts use **Chart.js** loaded from CDN in generated HTML (network needed when opening the file in a browser).

## Workflow

### 1. Locate data

Use a workspace path the user provides or attachments they uploaded.

Supported inputs: `.csv`, `.json` (array of objects), `.xlsx` / `.xls` (with pandas).

### 2. Inspect columns

```bash
python "<skill-base-dir>/scripts/visualize.py" --action inspect --input path/to/data.csv
```

Review `numeric_columns` vs `categorical_columns` before charting.

### 3. Choose rendering mode

| Mode | When | Command |
| --- | --- | --- |
| `auto_chart` | User did not specify chart type | `--action auto_chart --input ... --output report.html` |
| `chart` | Explicit axes and type | `--action chart --chart-type line --x month --y revenue --input ...` |
| `dashboard` | Multiple metrics | `--action dashboard --input ... --spec dashboard.json --output dashboard.html` |

Chart types: `bar`, `line`, `pie`, `scatter`.

Examples: [request-patterns.md](references/request-patterns.md). Dashboard spec: [dashboard-spec.md](references/dashboard-spec.md).

### 4. Present results

Tell the user:

- output HTML **absolute path**
- chart type and columns used
- how to open (browser, or `browser` builtin `navigateToUrl` with `file://` when allowed)

Format tips: [output-format.md](references/output-format.md).

## Guidelines

- **Inspect first** on unknown files — avoid wrong axis guesses
- **auto_chart** is a default, not magic — override with explicit `chart` when the user names columns
- **Keep data local** — do not upload user data to external chart services
- **Large files** — sample or aggregate in workspace before charting (>10k rows)
- **Locale** — titles/labels may match user language; column names stay as in the file

## Script actions

| Action | Purpose |
| --- | --- |
| `inspect` | Schema summary |
| `auto_chart` | Infer bar/line/pie/scatter |
| `chart` | Explicit chart |
| `dashboard` | Multi-chart HTML |

## References

- [request-patterns.md](references/request-patterns.md)
- [dashboard-spec.md](references/dashboard-spec.md)
- [output-format.md](references/output-format.md)
- [error-handling.md](references/error-handling.md)
