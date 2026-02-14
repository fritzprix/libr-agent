# Open Source Launch Manifesto

## What this project is

LibrAgent is not "yet another chat wrapper."
It is a local-first agent runtime designed to do real work with real tools.

The goal is simple:

- remove setup friction,
- keep tool state visible,
- make autonomous workflows operable.

---

## What we optimize for

1. **Operational clarity over demo magic**
   - If a workflow fails, users should know why.
   - If a workflow succeeds, users should know what actually happened.

2. **Mainstream surface, expert core**
   - Normal users get clean defaults.
   - Advanced users get power without hacks.

3. **Stateful intelligence**
   - Browser/terminal/workspace context must survive across steps.
   - Agents should reason from current state, not guess from memory.

4. **Guardrails before scale**
   - Recursive capabilities require limits and controls.
   - Depth/fanout/budget are part of product quality, not optional extras.

---

## Engineering stance

- We fix root causes.
- We keep architecture explainable.
- We validate with compile/lint/tests before confidence.
- We prefer explicit contracts over hidden behavior.

This is why the codebase invests in:

- session lineage and tree visibility,
- builtin + MCP interoperability,
- typed service boundaries,
- Rust-orchestrated agent workflows.

---

## Community promise

If you open an issue, submit a PR, or review design trade-offs:

- You will get direct technical reasoning.
- You will see decisions documented, not hand-waved.
- You will be treated as a collaborator, not a content consumer.

We welcome:

- bug reports with reproduction,
- architecture critiques with alternatives,
- sharp PRs that reduce complexity.

---

## The bar

We are not here to look smart.
We are here to build an agent platform that remains understandable when it gets powerful.

If a change improves reliability, clarity, and operator control, it belongs.
If it only looks flashy in a demo, it does not.

---

## One-line principle

Build with engineering discipline, ship with user empathy, operate with zero illusions.
