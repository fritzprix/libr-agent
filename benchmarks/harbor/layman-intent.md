# Layman intent — P0 routing suite

Checks whether LibrAgent routes plain, non-technical Korean requests to the right
system skill and tool. The suite covers 19 P0 cases (T01–T19) from
[#2183](https://github.com/fritzprix/libr-agent/issues/2183). Task directory:
[`libragent-layman-intent/`](./libragent-layman-intent/).

## Case categories

- **P** (proactive): the agent must pick the expected skill and tool without help.
- **Q** (question): the request is ambiguous. The agent must ask a clarifying
  question and must not act.
- **N** (negative): a simple task. The agent must do it directly and must not
  over-orchestrate (for example, it must not call `__spawnSession`).

## Grading

Most cases are about routing, not about a file the agent leaves behind. Those
tasks have a placeholder verifier that always gives reward `1`. Do not read
Harbor's reward as the score for them.

The score comes from the trajectory checker
`scripts/check-layman-intent-trajectories.py` (from #2227). It reads each trial's
ATIF trajectory and checks the skill read, the required and forbidden tool
calls, and the key arguments.

T04, T14 and T15 also have real artifact verifiers (build log, summary that
covers all 50 docs, README typo fix). For those, Harbor's reward is meaningful
as well.

| #   | Task                          | Category | Expected skill      | Network    |
| --- | ----------------------------- | -------- | ------------------- | ---------- |
| T01 | `t01-water-reminder`          | P        | loop                | no-network |
| T02 | `t02-watch-folder-mornings`   | P        | loop                | no-network |
| T03 | `t03-weekday-summary`         | P        | schedule            | no-network |
| T04 | `t04-build-done-notify`       | P        | call-me-back        | no-network |
| T05 | `t05-tell-me-later`           | Q        | —                   | no-network |
| T06 | `t06-stock-data`              | P        | mcp-installer       | public     |
| T07 | `t07-connect-npx-package`     | P        | mcp-installer       | public     |
| T08 | `t08-install-github-skill`    | P        | skill-deployer      | public     |
| T09 | `t09-install-github-mcp`      | P        | mcp-installer       | public     |
| T10 | `t10-install-that`            | Q        | —                   | no-network |
| T11 | `t11-command-not-found`       | P        | setup-wizard        | public     |
| T12 | `t12-save-organizing-routine` | P        | skill-creator       | no-network |
| T13 | `t13-hand-off-task`           | P        | delegate            | no-network |
| T14 | `t14-summarize-50-docs`       | P        | divide-conquer      | no-network |
| T15 | `t15-readme-typo`             | N        | —                   | no-network |
| T16 | `t16-build-a-team`            | P        | teamwork            | no-network |
| T17 | `t17-stock-assistant`         | P        | recruit             | no-network |
| T18 | `t18-recall-deploy-time`      | P        | context-recall      | no-network |
| T19 | `t19-remember-today`          | P        | knowledge-distiller | no-network |

Each task directory has a `README.md` with the prompt and the expected behavior.

## Running

With LibrAgent running (see [README](./README.md)):

```sh
export PYTHONPATH="$(pwd)"
harbor run -p benchmarks/harbor/libragent-layman-intent \
  -a benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter \
  --ak api_url=http://localhost:3030/api \
  --jobs-dir jobs/layman-intent
python3 scripts/check-layman-intent-trajectories.py jobs/layman-intent/<job-id>
```

## Limits

- T18 needs a host-side compacted session fixture so there is history to recall.
  The task has no such fixture yet, so a T18 failure can come from the
  environment, not the routing.
- There is no `dataset.toml`. Harbor computes its task digests only on
  `harbor add` / `harbor publish`. Add it when the suite is published to the Hub.
- This PR adds only the task definitions. Running the bench and any host
  infrastructure are out of scope.
