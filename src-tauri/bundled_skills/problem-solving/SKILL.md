---
name: problem-solving
description: Systematic framework for analyzing and solving complex problems. Use when facing unfamiliar challenges, debugging issues, making architectural decisions, or when initial approaches fail.
---

# Problem Solving Framework

A systematic approach to tackling complex problems when you can't just "know" the answer.

## The Core Loop

```
OBSERVE → ORIENT → DECIDE → ACT → VERIFY
   ↑                                    |
   └────────────────────────────────────┘
```

Every problem follows this cycle. The key is knowing which phase you're in and not skipping steps.

## Phase 1: OBSERVE (Gather Evidence)

**Goal:** Understand the actual situation, not what you assume.

### Questions to Ask
- What exactly is happening? (Symptoms)
- What was expected to happen? (Expectations)
- When did this start? (Timeline)
- What changed recently? (Triggers)
- What has already been tried? (History)

### Evidence Gathering Tools
```
readFile         → Examine source code, configs, logs
searchFiles      → Find relevant files
searchLineInFile → Locate specific patterns
runShell         → Check system state
extractWebContent → Research external docs
getCurrentState  → Review your planning state
```

### Rules
- **Read before you theorize.** Don't guess what a file contains.
- **Save what you find.** Use scratchpad for critical discoveries.
- **Stay focused.** Investigate one hypothesis at a time.

## Phase 2: ORIENT (Analyze & Hypothesize)

**Goal:** Form a theory about root cause.

### Techniques

#### Technique 1: Divide and Conquer
```
Problem scope: 100 files
→ Which subsystem? → 10 files
→ Which module? → 3 files
→ Which function? → 1 function
→ Which line? → ROOT CAUSE
```

#### Technique 2: Binary Search
```
"It worked before, doesn't work now"
→ What changed between then and now?
→ Test midpoint
→ Narrow to half that changed
→ Repeat
```

#### Technique 3: Trace Forward
```
Input → Step 1 → Step 2 → Step 3 → Output
                    ↑
              Where does it diverge from expected?
```

#### Technique 4: Trace Backward
```
Bad output ← What produced this? ← What fed that? ← ROOT CAUSE
```

#### Technique 5: Elimination
```
Possible causes: A, B, C, D
Test A → Not the issue → Eliminate
Test B → Not the issue → Eliminate  
Test C → REPRODUCES → Deep dive into C
```

### Use pauseAndThink
```
pauseAndThink("I see symptoms X, Y, Z. 
Hypothesis A: ... because ...
Hypothesis B: ... because ...
Most likely: A, because evidence X directly supports it.
Next test: ...")
```

**Don't skip this.** Structured thinking prevents thrashing.

## Phase 3: DECIDE (Choose Approach)

**Goal:** Pick the best action with available information.

### Decision Matrix
```
| Option        | Effort | Risk | Confidence | Reversible? |
|---------------|--------|------|------------|-------------|
| Quick fix     | Low    | Med  | 60%        | Yes         |
| Refactor      | High   | Low  | 90%        | Yes         |
| Workaround    | Low    | High | 80%        | No          |
```

### Decision Rules
1. **High confidence + Low risk → Act immediately**
2. **Low confidence → Gather more evidence first**
3. **High risk → Make reversible changes, verify incrementally**
4. **Time pressure → Start with quickest reversible option**

### When Stuck: Rubber Duck It
```
critiqueAndReflection → "What am I trying to do? What have I tried? 
Why didn't it work? What haven't I considered?"
```

## Phase 4: ACT (Execute Solution)

**Goal:** Implement the chosen approach with precision.

### Execution Protocol
1. **Save current state** (in case you need to revert)
2. **Make smallest possible change**
3. **Verify immediately after change**
4. **Document what you did and why**

### Incremental Changes
```
❌ BAD: Rewrite entire module, hope it works
✅ GOOD: 
   Change 1 → verify → checkpoint
   Change 2 → verify → checkpoint  
   Change 3 → verify → done
```

### Pre-Flight Checklist
Before editing code:
- [ ] Read the current file/function
- [ ] Understand surrounding context
- [ ] Identify exact location of change
- [ ] Preview edit if unsure (previewReplacement)
- [ ] Have rollback plan

## Phase 5: VERIFY (Confirm Success)

**Goal:** Prove the solution actually works.

### Verification Methods
```
1. Direct test: Run the thing, see if it works
2. Edge cases: Try unusual inputs
3. Regression: Check nothing else broke
4. Read-back: Re-read changed code for correctness
```

### Verification ≠ Assumption
```
❌ "I edited the file, so it should work now"
✅ "I edited the file, ran the tests, and all 47 pass"
```

## Failure Recovery

### When Your Fix Didn't Work
1. **Don't panic.** This is normal.
2. **Revert** if change made things worse
3. **Re-examine** your hypothesis
4. **Check assumptions** — something you "knew" might be wrong
5. **Try different approach** from your decision matrix

### When You're Going in Circles
```
STOP.

1. critiqueAndReflection → "What am I repeating?"
2. Save all current findings to scratchpad
3. List what you've tried and eliminated
4. Identify the ACTUAL unknown (not the symptom)
5. Attack the unknown directly
```

### When You're Truly Stuck
1. **Step back:** Is the problem correctly defined?
2. **Widen scope:** Are there adjacent clues you missed?
3. **Change perspective:** What would a different specialist see?
4. **Delegate:** Can another agent help? (createChildSession)
5. **Ask the user:** Sometimes domain knowledge is needed

## Problem Types Quick Reference

### Bug Fixing
```
OBSERVE: Reproduce → read error → trace to source
ORIENT:  Identify root cause vs symptom
DECIDE:  Fix root cause (not band-aid)
ACT:     Minimal change at root
VERIFY:  Bug gone + no regressions
```

### Feature Building
```
OBSERVE: Understand requirements → existing code → constraints
ORIENT:  Design approach → identify components
DECIDE:  Architecture choice
ACT:     Build incrementally, test each piece
VERIFY:  Requirements met + edge cases handled
```

### Performance Issues
```
OBSERVE: Measure (don't guess!) → identify bottleneck
ORIENT:  Profile → find hot path
DECIDE:  Optimize bottleneck (not random code)
ACT:     Change one thing → measure again
VERIFY:  Measurable improvement + correctness maintained
```

### Configuration/Environment
```
OBSERVE: What's installed? What versions? What's configured?
ORIENT:  Compare expected vs actual environment
DECIDE:  Fix gap between expected and actual
ACT:     Install/configure missing pieces
VERIFY:  System runs correctly end-to-end
```

## The Meta-Rule

**If you've spent more than 3 attempts on the same approach without progress, you're on the wrong track.** 

Stop. Think. Try something fundamentally different.
