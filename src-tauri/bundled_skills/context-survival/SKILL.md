---
name: context-survival
description: Master techniques for operating effectively within limited context windows. Use when handling complex multi-step tasks, managing conversation memory, preventing information loss, or optimizing token usage for maximum productivity.
---

# Context Survival Guide

You are operating in a limited context window. This guide helps you work effectively despite memory constraints.

## The Core Problem

Your context window is finite. Every token of input + output counts. When context fills up:
- Earlier conversation details get pushed out
- You lose track of decisions, IDs, paths, and progress
- You repeat work already done
- Quality degrades as you lose the thread

## Survival Strategy: External Brain Pattern

**Principle:** Your tools ARE your long-term memory. Use them.

### 1. Planning = Your Task Memory

```
createGoal → "What am I trying to accomplish?"
addTodo    → Each discrete step
checkTodo  → Track completion
```

**Rule:** If a task has 3+ steps, ALWAYS create a goal and todos first.

### 2. Scratchpad = Your Working Memory

```
addScratchpad → Save critical findings (max 10 items)
readScratchpad → Recall what you discovered
updateScratchpad → Keep info current
```

**What to save:**
- File paths you'll need again
- IDs (process, session, resource)
- Key decisions and their rationale
- Intermediate results
- Error patterns you've seen

**What NOT to save:**
- Obvious facts
- Things you can re-derive cheaply
- Verbose logs (summarize instead)

### 3. Knowledge = Your Long-Term Memory

```
saveKnowledge → Persist across sessions
searchKnowledge → Find past learnings
```

**Use for:**
- Patterns you discover (reusable across tasks)
- Environment-specific info (paths, configs)
- Lessons learned from failures
- Architecture decisions

## Token Economy Rules

### Rule 1: Read Once, Save Critical

```
❌ BAD: Read file → forget → read again → forget → read again
✅ GOOD: Read file → save key info to scratchpad → reference scratchpad
```

### Rule 2: Summarize, Don't Hoard

```
❌ BAD: Save 500 lines of log output
✅ GOOD: Save "Error at line 42: null pointer in auth.rs, caused by missing token validation"
```

### Rule 3: Plan Before Acting

```
❌ BAD: Jump into code → get lost → start over
✅ GOOD: createGoal → addTodo (3-5 steps) → execute systematically
```

### Rule 4: Checkpoint Progress

After completing each major step:
1. `checkTodo` - Mark done
2. `addScratchpad` - Save key output
3. Review remaining todos

### Rule 5: One Thread at a Time

```
❌ BAD: Start investigating A, switch to B, remember C, lose track of all
✅ GOOD: Finish A completely → checkpoint → move to B
```

## Emergency Protocols

### "I Lost Track" Recovery

1. `getCurrentState` → Read your planning state
2. `listScratchpad` → Check what you saved
3. `searchKnowledge` → Look for past context
4. Reconstruct situational awareness from saved state
5. Resume from last checkpoint

### "Context Getting Full" Indicators

- You're repeating questions already answered
- Tool outputs are getting truncated
- You can't remember what step you're on

**Action:** 
1. Save ALL critical state to scratchpad immediately
2. Summarize progress to user
3. If multi-session: save to knowledge for next session

### "Complex Task" Startup Sequence

Before diving in:
```
1. createGoal("Clear objective statement")
2. addTodo("Step 1: ...") through addTodo("Step N: ...")
3. addScratchpad("Key constraints: ...")
4. THEN start execution
```

## Anti-Patterns

| Anti-Pattern | Why It Fails | Better Approach |
|---|---|---|
| Never using planning | Lose track after 3 steps | Always plan for complex tasks |
| Saving everything | Scratchpad bloat, 10 item limit | Save only what you'll reference |
| Ignoring past knowledge | Repeat expensive investigations | searchKnowledge first |
| Parallel investigations | Context fragmentation | Sequential, with checkpoints |
| No checkpointing | Single failure = restart from scratch | Save progress after each step |

## Quick Reference

```
STARTING A TASK:
  createGoal → addTodo(s) → execute step by step

DURING EXECUTION:
  Save discoveries → addScratchpad
  Finish step → checkTodo
  Need past info → readScratchpad / searchKnowledge

WRAPPING UP:
  Important learning → saveKnowledge (persists!)
  Clear temporary data → clearScratchpad

LOST? → getCurrentState → listScratchpad → reconstruct
```

## The Golden Rule

**If you'll need information again later, save it NOW. Your future self has no memory of this moment.**
