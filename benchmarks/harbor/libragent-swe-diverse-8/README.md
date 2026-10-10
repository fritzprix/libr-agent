# fritzprix/libragent-swe-diverse-8

Curated Harbor Hub dataset: 8 easy SWE-bench Verified tasks (one repo each,
`<15 min fix`). For LibrAgent hourly coding-loop iteration — not a Verified
solve-rate estimate.

See [`../swe-diverse-8.md`](../swe-diverse-8.md) for selection rules.

```sh
pnpm bench:swe
# or
harbor run -d fritzprix/libragent-swe-diverse-8 -a <agent> -m <model>
```

Publish / update:

```sh
harbor publish benchmarks/harbor/libragent-swe-diverse-8 --public -t v0.1
```
