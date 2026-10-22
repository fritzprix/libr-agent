#!/bin/bash
# Oracle: the expected behavior is a routing decision inside LibrAgent
# (`__globFiles` with `pre_compaction_epoch_` → `__readFile` with `pre_compaction_epoch_` (last call succeeds)); there is no container artifact to produce.
echo "graded from the agent trajectory; nothing to write"
