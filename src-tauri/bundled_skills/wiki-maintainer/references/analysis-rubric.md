# Analysis rubric

## What to extract from `history__*`

| Signal | Why it matters |
| --- | --- |
| Repeated identical tool errors | Recovery / hint / schema gap |
| Long loops with no progress | Missing stop condition or wrong tool boundary |
| User correction after agent confidence | Interpretation or skill guidance gap |
| Missing prerequisite then install spiral | Skill dependency check incomplete |
| Success after one clarifying strategy | Candidate for a durable pattern |

## Evidence discipline

- One session → `draft` + hypothesis language.
- Same divergence in ≥2 sessions → stronger; may set `active`.
- Only mark “confirmed” after a gated skill change and a later successful session.

## Counterexamples

Before blaming a skill or tool contract, find a session where the same tools succeeded. If found, narrow scope (environment, ordering, missing preflight)—do not write a universal rule.
