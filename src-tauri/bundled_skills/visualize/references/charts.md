# Charts from tabular data

Turn **the user's** CSV, Excel, or JSON into interactive HTML charts (Chart.js via CDN). Not for Mermaid/mindmaps — those stay in [diagrams.md](diagrams.md).

## Path conventions

Replace `<skill-base-dir>` with this skill's absolute Base Directory (`visualize`).

## Prerequisites

CSV and JSON work with the Python standard library.

For Excel (`.xlsx`):

```bash
pip install pandas openpyxl
```

Opening generated HTML needs network for the Chart.js CDN.

## Workflow

### 1. Locate data

Workspace path from the user or uploaded attachments.

Supported: `.csv`, `.json` (array of objects), `.xlsx` / `.xls` (with pandas).

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

Examples: [charts-request-patterns.md](charts-request-patterns.md). Spec: [charts-dashboard-spec.md](charts-dashboard-spec.md).

### 4. Present results

Tell the user:

- output HTML **absolute path**
- chart type and columns used
- how to open (browser, or `browser__navigateToUrl` with `file://` when allowed)

Format tips: [charts-output-format.md](charts-output-format.md). Errors: [charts-error-handling.md](charts-error-handling.md).

## Guidelines

- **Inspect first** on unknown files
- **auto_chart** is a default — override when the user names columns
- **Keep data local** — do not upload to external chart services
- **Large files** — sample or aggregate before charting (>10k rows)
- **Locale** — titles/labels may match user language; column names stay as in the file

## Script actions

| Action | Purpose |
| --- | --- |
| `inspect` | Schema summary |
| `auto_chart` | Infer bar/line/pie/scatter |
| `chart` | Explicit chart |
| `dashboard` | Multi-chart HTML |
