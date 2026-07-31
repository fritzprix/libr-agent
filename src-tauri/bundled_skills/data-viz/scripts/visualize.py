#!/usr/bin/env python3
"""
LibrAgent data visualization — CSV/Excel/JSON to standalone HTML charts.

Actions:
  inspect       — column names, dtypes, row count
  auto_chart    — infer chart type and render
  chart         — explicit chart type
  dashboard     — multiple charts from a JSON spec file

Output: standalone HTML (Chart.js via CDN). JSON result on stdout.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any


def error_exit(message: str, code: int = 1) -> None:
    print(json.dumps({"error": message}), file=sys.stderr)
    sys.exit(code)


def load_rows(path: Path) -> tuple[list[str], list[dict[str, Any]]]:
    suffix = path.suffix.lower()
    if suffix == ".csv":
        with path.open(newline="", encoding="utf-8-sig") as handle:
            reader = csv.DictReader(handle)
            if not reader.fieldnames:
                error_exit("CSV has no header row.")
            rows = [dict(row) for row in reader]
            return list(reader.fieldnames), rows
    if suffix in {".xlsx", ".xls"}:
        try:
            import pandas as pd
        except ImportError:
            error_exit(
                "pandas is required for Excel files. Run: pip install pandas openpyxl"
            )
        frame = pd.read_excel(path)
        rows = frame.to_dict(orient="records")
        return [str(col) for col in frame.columns], rows
    if suffix == ".json":
        data = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(data, list):
            if not data:
                error_exit("JSON array is empty.")
            if not isinstance(data[0], dict):
                error_exit("JSON must be an array of objects.")
            columns = list(data[0].keys())
            return columns, data
        error_exit("JSON must be an array of objects for tabular charts.")
    error_exit(f"Unsupported input format: {suffix}")


def coerce_numeric(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    text = str(value).strip().replace(",", "")
    if not text:
        return None
    try:
        return float(text)
    except ValueError:
        return None


def is_numeric_column(rows: list[dict[str, Any]], column: str) -> bool:
    samples = [coerce_numeric(row.get(column)) for row in rows[:50]]
    nums = [v for v in samples if v is not None]
    return len(nums) >= max(1, len(samples) // 2)


def infer_chart(columns: list[str], rows: list[dict[str, Any]]) -> dict[str, Any]:
    numeric = [c for c in columns if is_numeric_column(rows, c)]
    categorical = [c for c in columns if c not in numeric]
    if len(categorical) >= 1 and len(numeric) >= 1:
        return {
            "type": "bar",
            "x": categorical[0],
            "y": numeric[0],
            "title": f"{numeric[0]} by {categorical[0]}",
        }
    if len(numeric) >= 2:
        return {
            "type": "scatter",
            "x": numeric[0],
            "y": numeric[1],
            "title": f"{numeric[1]} vs {numeric[0]}",
        }
    if len(numeric) == 1 and len(rows) <= 12:
        return {
            "type": "pie",
            "x": columns[0],
            "y": numeric[0],
            "title": numeric[0],
        }
    error_exit("Could not infer chart type. Use --action chart with explicit columns.")


def build_dataset(rows: list[dict[str, Any]], x_col: str, y_col: str, chart_type: str) -> dict[str, Any]:
    labels: list[str] = []
    values: list[float] = []
    points: list[dict[str, float]] = []

    for row in rows:
        x_val = row.get(x_col, "")
        y_num = coerce_numeric(row.get(y_col))
        if y_num is None:
            continue
        if chart_type == "scatter":
            x_num = coerce_numeric(x_val)
            if x_num is None:
                continue
            points.append({"x": x_num, "y": y_num})
        else:
            labels.append(str(x_val))
            values.append(y_num)

    if chart_type == "scatter":
        return {"label": y_col, "data": points}
    return {"label": y_col, "data": values, "labels": labels}


def render_html(
    *,
    title: str,
    chart_type: str,
    dataset: dict[str, Any],
    output: Path,
) -> None:
    labels = dataset.get("labels", [])
    data = dataset["data"]
    label = dataset["label"]

    if chart_type == "scatter":
        chart_js_type = "scatter"
        chart_data = {"datasets": [{"label": label, "data": data}]}
    else:
        chart_js_type = chart_type
        chart_data = {
            "labels": labels,
            "datasets": [
                {
                    "label": label,
                    "data": data,
                    "backgroundColor": "rgba(54, 162, 235, 0.5)",
                    "borderColor": "rgba(54, 162, 235, 1)",
                    "borderWidth": 1,
                }
            ],
        }

    scales = (
        '{"x": {"beginAtZero": true}, "y": {"beginAtZero": true}}'
        if chart_js_type in {"bar", "line", "scatter"}
        else "{}"
    )

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js"></script>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem; background: #fafafa; color: #111; }}
    h1 {{ font-size: 1.25rem; margin-bottom: 1rem; }}
    .card {{ background: #fff; border-radius: 12px; padding: 1rem; box-shadow: 0 2px 8px rgba(0,0,0,.08); }}
    canvas {{ max-height: 480px; }}
  </style>
</head>
<body>
  <div class="card">
    <h1>{title}</h1>
    <canvas id="chart"></canvas>
  </div>
  <script>
    new Chart(document.getElementById('chart'), {{
      type: {json.dumps(chart_js_type)},
      data: {json.dumps(chart_data)},
      options: {{
        responsive: true,
        plugins: {{ legend: {{ position: 'top' }} }},
        scales: {scales}
      }}
    }});
  </script>
</body>
</html>
"""
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(html, encoding="utf-8")


def render_dashboard(spec: dict[str, Any], rows: list[dict[str, Any]], output: Path) -> None:
    charts = spec.get("charts", [])
    if not charts:
        error_exit("Dashboard spec must include a non-empty charts array.")

    sections = []
    scripts = []
    for index, chart in enumerate(charts):
        chart_type = chart.get("type", "bar")
        x_col = chart["x"]
        y_col = chart["y"]
        title = chart.get("title", f"Chart {index + 1}")
        dataset = build_dataset(rows, x_col, y_col, chart_type)
        canvas_id = f"chart_{index}"
        if chart_type == "scatter":
            payload = {"datasets": [{"label": dataset["label"], "data": dataset["data"]}]}
            js_type = "scatter"
        else:
            payload = {
                "labels": dataset.get("labels", []),
                "datasets": [{"label": dataset["label"], "data": dataset["data"]}],
            }
            js_type = chart_type
        sections.append(f'<div class="card"><h2>{title}</h2><canvas id="{canvas_id}"></canvas></div>')
        scripts.append(
            f"new Chart(document.getElementById('{canvas_id}'), {{ type: '{js_type}', data: {json.dumps(payload)}, options: {{ responsive: true }} }});"
        )

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>{spec.get('title', 'Dashboard')}</title>
  <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js"></script>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem; background: #f4f4f5; }}
    h1 {{ margin-bottom: 1rem; }}
    .grid {{ display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); }}
    .card {{ background: #fff; border-radius: 12px; padding: 1rem; box-shadow: 0 2px 8px rgba(0,0,0,.08); }}
    h2 {{ font-size: 1rem; margin: 0 0 .75rem; }}
  </style>
</head>
<body>
  <h1>{spec.get('title', 'Dashboard')}</h1>
  <div class="grid">{''.join(sections)}</div>
  <script>{''.join(scripts)}</script>
</body>
</html>
"""
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(html, encoding="utf-8")


def action_inspect(args: argparse.Namespace) -> dict[str, Any]:
    columns, rows = load_rows(Path(args.input))
    numeric = [c for c in columns if is_numeric_column(rows, c)]
    return {
        "action": "inspect",
        "input": args.input,
        "row_count": len(rows),
        "columns": columns,
        "numeric_columns": numeric,
        "categorical_columns": [c for c in columns if c not in numeric],
    }


def action_auto_chart(args: argparse.Namespace) -> dict[str, Any]:
    columns, rows = load_rows(Path(args.input))
    spec = infer_chart(columns, rows)
    return _render_chart(args, spec, columns, rows)


def action_chart(args: argparse.Namespace) -> dict[str, Any]:
    if not args.x or not args.y:
        error_exit("--x and --y are required for chart action.")
    spec = {
        "type": args.chart_type,
        "x": args.x,
        "y": args.y,
        "title": args.title or f"{args.y} by {args.x}",
    }
    columns, rows = load_rows(Path(args.input))
    return _render_chart(args, spec, columns, rows)


def _render_chart(
    args: argparse.Namespace,
    spec: dict[str, Any],
    columns: list[str],
    rows: list[dict[str, Any]],
) -> dict[str, Any]:
    for col in (spec["x"], spec["y"]):
        if col not in columns:
            error_exit(f"Column not found: {col}")
    dataset = build_dataset(rows, spec["x"], spec["y"], spec["type"])
    output = Path(args.output or f"chart_{spec['type']}.html")
    render_html(title=spec["title"], chart_type=spec["type"], dataset=dataset, output=output)
    return {
        "action": args.action,
        "chart_type": spec["type"],
        "x": spec["x"],
        "y": spec["y"],
        "title": spec["title"],
        "output": str(output.resolve()),
        "row_count": len(rows),
    }


def action_dashboard(args: argparse.Namespace) -> dict[str, Any]:
    columns, rows = load_rows(Path(args.input))
    spec = json.loads(Path(args.spec).read_text(encoding="utf-8"))
    output = Path(args.output or "dashboard.html")
    render_dashboard(spec, rows, output)
    return {
        "action": "dashboard",
        "output": str(output.resolve()),
        "charts": len(spec.get("charts", [])),
        "row_count": len(rows),
        "columns": columns,
    }


ACTIONS = {
    "inspect": action_inspect,
    "auto_chart": action_auto_chart,
    "chart": action_chart,
    "dashboard": action_dashboard,
}


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="LibrAgent data visualization")
    parser.add_argument("--action", required=True, choices=sorted(ACTIONS))
    parser.add_argument("--input", required=True, help="CSV, XLSX, or JSON path")
    parser.add_argument("--output", help="Output HTML path")
    parser.add_argument("--x", help="Label/category column")
    parser.add_argument("--y", help="Value column")
    parser.add_argument("--title")
    parser.add_argument(
        "--chart-type",
        choices=["bar", "line", "pie", "scatter"],
        default="bar",
    )
    parser.add_argument("--spec", help="Dashboard JSON spec path")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.action == "dashboard" and not args.spec:
        error_exit("--spec is required for dashboard action.")
    try:
        payload = ACTIONS[args.action](args)
    except SystemExit:
        raise
    except Exception as exc:
        error_exit(str(exc))
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
