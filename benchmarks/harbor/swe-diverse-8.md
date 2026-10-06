# SWE-bench Verified — diverse-8 (local iteration suite)

Fast coding-agent loop for LibrAgent harness work: **find bug → edit code → pass
issue tests**. Published on Harbor Hub as
[`fritzprix/libragent-swe-diverse-8`](https://hub.harborframework.com/datasets/fritzprix/libragent-swe-diverse-8)
(`@v0.1`). Tasks are pinned digests of `swe-bench/*` packages from
`swe-bench/swe-bench-verified`. Manifest:
[`libragent-swe-diverse-8/dataset.toml`](./libragent-swe-diverse-8/dataset.toml).

| Script | Tasks | Intent |
| --- | --- | --- |
| `pnpm bench:swe` | Hub dataset `@v0.1` (8 tasks) | ~1h warm-cache iteration |
| `pnpm bench:swe:n1` | first task only | smoke / plumbing |
| `pnpm bench:swe:all` | `swe-bench/swe-bench-verified` (500) | rare full sweep |

## Selection rules

- Difficulty label **`<15 min fix`** (OpenAI / SWE-bench Verified annotation)
- One instance per repository (repo diversity)
- Prefer small `FAIL_TO_PASS` (≤2) and modest `PASS_TO_PASS` / test patch size
- Exclude known weak-coverage / flaky IDs from community denylists (e.g. UTBoost
  / KNOWN_BAD), plus Harbor outlier `scikit-learn__scikit-learn-14710`

This set is **not** a solve-rate estimate for Verified. It is a diversified
plumbing + coding-loop probe for repeated improve → rerun cycles.

## Tasks

| # | `instance_id` | Repo | Notes |
| --- | --- | --- | --- |
| 1 | `pallets__flask-5014` | pallets/flask | Tiny validation fix; good smoke |
| 2 | `pytest-dev__pytest-5809` | pytest-dev/pytest | Small pastebin test |
| 3 | `psf__requests-1142` | psf/requests | Compact HTTP client fix |
| 4 | `pylint-dev__pylint-6903` | pylint-dev/pylint | Runner edge case |
| 5 | `sphinx-doc__sphinx-9711` | sphinx-doc/sphinx | Extension metadata |
| 6 | `django__django-12419` | django/django | Middleware / project template |
| 7 | `sympy__sympy-20916` | sympy/sympy | Small math API fix |
| 8 | `scikit-learn__scikit-learn-14141` | scikit-learn/scikit-learn | `show_versions` deps |

## Runtime expectations

- **Cold start**: first pull of per-repo Docker images can exceed 1 hour.
- **Warm cache** (`~/.cache/harbor` + images already present): aim for ~1 hour
  wall clock at `-k 1 -n 1` with a mid-tier coding model.
- Official Harbor timeouts/resources are left unchanged (submission-compatible).

## Analyze a run

```sh
python .agents/skills/jobs-trace-analyzer/scripts/analyze_jobs_trace.py \
  jobs/<timestamp> \
  --output .libragent/work/trace-analysis/<timestamp>-swe-diverse-8.md
```

For BM → fix → rerun cycles, use `harbor-harness-improvement-loop`.
