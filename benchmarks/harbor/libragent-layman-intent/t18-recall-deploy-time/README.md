# T18 — context-recall

- Category: **P**
- Prompt: 아까 정한 배포 시간 뭐였지?
- Expected: read the compacted transcript; do not invent an answer
- Network: `no-network`
- Note: Needs a host session fixture whose earlier (compacted) turns set the deploy time. Without it, a pass means the agent says it cannot find it instead of inventing one.
