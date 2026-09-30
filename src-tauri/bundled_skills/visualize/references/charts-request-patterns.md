# Request Patterns

| User says | Action | Example |
| --- | --- | --- |
| 이 CSV 차트로 / plot this | `inspect` → `auto_chart` | `--output sales_chart.html` |
| 월별 매출 트렌드 | `chart` line | `--x month --y revenue --chart-type line` |
| 카테고리별 비교 | `chart` bar | `--x category --y amount` |
| 비율 파이차트 | `chart` pie | `--x region --y share` |
| 상관관계 | `chart` scatter | `--x price --y units` |
| 대시보드 만들어줘 | `dashboard` | `--spec dashboard.json` |

## Natural language → columns

After `inspect`, map user terms to actual column names (case-sensitive).

If ambiguous, ask once: "Which column is revenue — `sales` or `revenue_usd`?"
