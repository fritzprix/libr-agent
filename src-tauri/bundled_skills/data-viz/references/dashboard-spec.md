# Dashboard Spec

JSON file passed to `--spec` for `--action dashboard`.

```json
{
  "title": "Sales overview",
  "charts": [
    {
      "title": "Revenue by month",
      "type": "line",
      "x": "month",
      "y": "revenue"
    },
    {
      "title": "Units by region",
      "type": "bar",
      "x": "region",
      "y": "units"
    }
  ]
}
```

Rules:

- All charts read from the same `--input` table
- `type`: `bar`, `line`, `pie`, or `scatter`
- `x` / `y` must exist in the input file
