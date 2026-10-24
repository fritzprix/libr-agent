# fritzprix/libragent-org-collab-7

Harbor Hub dataset for **explicit Org teamwork** + multi-agent tool collaboration
(LibrAgent `teamwork` → `createOrg` → `spawnSession` / `checkSession`).

| Item       | Value                                                                                                           |
| ---------- | --------------------------------------------------------------------------------------------------------------- |
| Dataset    | [`fritzprix/libragent-org-collab-7`](https://hub.harborframework.com/datasets/fritzprix/libragent-org-collab-7) |
| Tag        | `v0.2` (+ `latest`)                                                                                             |
| Tasks      | 7                                                                                                               |
| Visibility | public                                                                                                          |

## Tasks

| ID  | Task                            | Coord      | Probe                                            |
| --- | ------------------------------- | ---------- | ------------------------------------------------ |
| T1  | `org-notes-api-release-v1`      | Hub-spoke  | Full spine + API release                         |
| T2  | `org-research-spec-impl-v1`     | Sequential | Research→SPEC→impl gates                         |
| T3  | `org-parallel-feature-merge-v1` | Hub-spoke  | Parallel merge (≥3 children; root may integrate) |
| T4  | `org-exploratory-bug-hunt-v1`   | Swarm      | Bug hunt board                                   |
| T5  | `org-docs-site-ship-v1`         | Hub-spoke  | Docs ship                                        |
| T6  | `org-rework-idle-reuse-v1`      | Sequential | Reject + Idle reuse (≥1 Implementer child)       |
| T7  | `org-bootstrap-trap-v1`         | Hub-spoke  | Bootstrap trap recovery                          |

Each task requires `/app/ORG_HARNESS_REPORT.json` (`orgId`, children, status) plus
task-specific product evidence. Trajectory review still owns “was Org used correctly?”.

### v0.2 verifier alignment (P3)

- **T6:** Allows a single Implementer child; requires `rejectCount ≥ 1` and
  `implementerSessionId` ∈ `children` (Idle reuse is not punished by a ≥2 pad).
- **T3:** Requires ≥3 distinct-role children (Worker A/B + Tester); Integrator
  child optional when the root integrates.

## Run

```bash
harbor download fritzprix/libragent-org-collab-7@v0.2
pnpm bench:org:n1          # first task smoke
pnpm bench:org             # full 7-task suite
```

Local authoring tree (gitignored work copy):
`.libragent/work/harbor-datasets/libragent-org-collab-7/`.

Protocol / scorecard (libr-delegate oriented):
`.libragent/work/org-collab-eval/20261011-protocol/`.
