# Layman intent — P0 routing suite

Checks whether LibrAgent routes plain, non-technical Korean requests to the right
system skill and tool. The suite covers 19 P0 cases (T01–T19) from
[#2183](https://github.com/fritzprix/libr-agent/issues/2183). Task directory:
[`libragent-layman-intent/`](./libragent-layman-intent/).

## Case categories

- **P** (proceed): the agent must pick the expected skill and finish with its
  critical-path tool, stating any assumption instead of asking. No task expects
  a clarifying question as the answer.
- **N** (negative): a simple task. The agent must do it directly and must not
  over-orchestrate (for example, it must not call `__spawnSession`).

## Grading

Each task is judged only by its skill's critical path, as defined in
[`layman_intent_critical_path.py`](./layman_intent_critical_path.py). A task
passes when the trajectory has every critical-path tool call and the last call
matching each step succeeded. Success comes from the `tool_error` flag the
adapter writes into each ATIF observation's `extra`. N tasks pass when no
forbidden tool was called. Deeper analysis is left to an agent-based eval loop.

Two graders use the same rules:

- **Harbor reward.** Each task's `tests/test_outputs.py` grades
  `/logs/agent/trajectory.json`, which Harbor mounts from the trial's `agent/`
  directory. `tests/critical_path.py` is a copy of the shared module; a unit test
  keeps the copies in sync. T04, T14 and T15 also check their artifact (build
  log, summary of all 50 docs, README typo fix).
- **Host report.** `scripts/check-layman-intent-trajectories.py` grades a whole
  job and prints per-task, per-skill and overall routing accuracy. It matches
  trials to tasks by the `task_name` in each trial's `result.json`.

| #   | Task                          | Category | Expected skill      | Network    |
| --- | ----------------------------- | -------- | ------------------- | ---------- |
| T01 | `t01-water-reminder`          | P        | loop                | no-network |
| T02 | `t02-watch-folder-mornings`   | P        | loop                | no-network |
| T03 | `t03-weekday-summary`         | P        | schedule            | no-network |
| T04 | `t04-build-done-notify`       | P        | call-me-back        | no-network |
| T05 | `t05-tell-me-later`           | P        | loop                | no-network |
| T06 | `t06-stock-data`              | P        | mcp-installer       | public     |
| T07 | `t07-connect-npx-package`     | P        | mcp-installer       | public     |
| T08 | `t08-install-github-skill`    | P        | skill-deployer      | public     |
| T09 | `t09-install-github-mcp`      | P        | mcp-installer       | public     |
| T10 | `t10-install-that`            | P        | mcp-installer       | public     |
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
python3 scripts/check-layman-intent-trajectories.py --jobs-dir jobs/layman-intent --job <job-id>
```

## Limits

- T18 seeds `/app/.libragent/pre_compaction_epoch_{1,2}.md`. In host workspace
  mode the seed must be copied into the host workspace first.
- There is no `dataset.toml`. Harbor computes its task digests only on
  `harbor add` / `harbor publish`. Add it when the suite is published to the Hub.
- This PR adds only the task definitions. Running the bench and any host
  infrastructure are out of scope.
