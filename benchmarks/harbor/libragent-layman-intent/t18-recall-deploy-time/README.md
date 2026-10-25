# T18 — context-recall

- Category: **P**
- Prompt: 아까 정한 배포 시간 뭐였지?
- Expected: `__globFiles` with `pre_compaction_epoch_` → `__readFile` with `pre_compaction_epoch_2` (last call succeeds)
- Network: `no-network`
- Seed: `/app/.libragent/pre_compaction_epoch_{1,2}.md`; epoch 2 fixes the deploy time (목요일 오후 3시).
