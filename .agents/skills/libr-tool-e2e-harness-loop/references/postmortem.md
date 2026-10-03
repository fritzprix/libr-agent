# Postmortem template

Save as `postmortem.md` for the cycle.

```markdown
## Meta
- cycle-id:
- session id:
- app/health note:
- contract pointer: contract.md

## Verdict (from report)
PASS | FAIL | PARTIAL

## Matrix (normalized)
| tool | expected | observed | class |

## Timeline (first divergence)
1. intent → tool → observation
2. …
- **First bad observation:** (quote + message index/role)

## Root cause candidates
| id | class | layer | evidence | confidence |

## What is NOT the cause
- (explicitly rule out model flakiness unless tool contract is clean)

## Contamination
- YES/NO — evidence

## Reload / rebuild gates for next E2E
- [ ] desktop rebuild
- [ ] chrome extension Reload
- [ ] other:
```

## Class reminder

`precondition` | `contract_lie` | `capability_gap` | `schema_guidance` | `handler_bug` | `contamination` | `harness`
