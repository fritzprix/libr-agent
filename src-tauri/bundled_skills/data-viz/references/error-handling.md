# Error Handling

| Error | Cause | Action |
| --- | --- | --- |
| Column not found | Wrong `--x` / `--y` | Re-run `inspect` |
| pandas missing | Excel input | `pip install pandas openpyxl` |
| Could not infer chart | No clear numeric/categorical pair | Ask user for columns; use `chart` |
| Empty CSV | No rows | Verify file path and export |
| CDN blocked offline | Chart.js not loading | Open HTML when online or embed note for user |

Provide the corrected `visualize.py` command in the recovery step.
