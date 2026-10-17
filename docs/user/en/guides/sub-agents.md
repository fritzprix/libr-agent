---
title: Sub-agents & orchestration
description: 'Coordinate child sessions, delegation, and Org/teamwork patterns with bundled skills.'
---

# Sub-agents & orchestration

> A session can start **child sessions (sub-agents)** to split work.  
> Pick parallel, pipeline, or team patterns with bundled **orchestration skills**.

---

## What is a sub-agent?

|                   | Meaning                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Is**            | A **child session** created by the parent. The parent–child link shows in History.                                                    |
| **Is not**        | A clone of the parent. By default it uses a **separate work folder**; local skills and folder rules are **not** copied automatically. |
| **Who drives it** | Usually you attach `@skill:delegate` (etc.) so the agent handles handoff, progress, and merge.                                        |

Parents reuse an **idle child with the same role** when possible, and create a new child only when needed. Flow: **hand off → check progress → nudge → merge results**.  
Sibling children do not talk to each other — the parent relays.

You can set **task difficulty** (low / normal / high) when delegating. Under **Settings → AI & Models → Sub-Agent Model Routing**, pick models per level; if unset, the parent or default model is used. Prefer the lowest difficulty that still works.

### Where it shows in the UI

| Screen             | Content                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------ |
| **Chat / History** | Child count on the card. On delete: **Delete all** vs **Delete only this** (keep children) |
| Sidebar **Org**    | **Explicit teams** only. One-off delegation does **not** appear here                       |

![Org view](../../assets/screenshots/guides/org.png)

**Org export / import**: You can export a team as a `.libragent-org` bundle file from the Org view to back it up or move it to another machine. Use **Export Org** on an Org card to choose a destination folder, and **Import Org** at the top of the view to pick a bundle file. Import **merges** with what you already have, and sensitive data such as API keys is not included. Encrypted bundles cannot be imported from this screen yet, so export without a password.

---

## The principles

More than one agent can work together. LibrAgent keeps the same principles in all cases.

| Principle                             | Meaning                                                                                               |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| **On your computer, your choice**     | The work runs on your computer. The result stays as files. You choose the model and who does the work |
| **Start with the smallest structure** | If one agent is enough, the app does not make a team. It makes the structure bigger only when it must |
| **The parent is responsible**         | A child does only its own task. The parent decides if the result is used                              |
| **Proof, not words**                  | The agent does not stop at "I am done". It checks the work                                            |
| **The same flow everywhere**          | In every pattern, the order is the same: **hand off → check → merge → verify**                        |

---

## How it works

```text
              ┌──────────────── Parent chat ────────────────┐
              │ 1 split  2 hand off  3 check  4 merge  5 verify │
              └──────┬─────────────┬─────────────┬────────┘
                     ▼             ▼             ▼
                 Child chat A  Child chat B  Child chat C
          (each has its own assistant and work folder)
                 Child chats do not talk to each other.
```

When you add a skill, the agent works in this order. You do not do these steps yourself.

| Step      | What the agent does                                          | Good to know                                                                        |
| --------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Look      | Looks for a child that is **idle** and has the **same role** | If it finds one, it uses that child again. It does not make a new one               |
| Hand off  | Writes the task for the child                                | The child cannot see the parent chat. The task must have all the needed facts       |
| Wait      | Waits for the child, or does other work                      | For many tasks, the agent starts them all at the same time                          |
| Check     | Looks at the child: working or done                          | The agent can stop a child that is stuck                                            |
| Follow up | Asks the child to fix what is missing                        | For a new task, the agent can clear the old chat. The files in the work folder stay |
| Merge     | Puts the results together                                    | At the end, the parent checks the result again                                      |

### How does the result come back?

- The **last answer** of the child is the result.
- The parent **cannot see** the private notes of the child.
- A child can say only "I saved it to a file". If the work folders are separate, the parent cannot read that file. In your request, write "Put the result in your answer". Or make the child use the same work folder.
- A child can say "I am done". The parent does not accept this alone. The parent **checks** the work. For more information, read [Check the result before you accept it](#check-the-result-before-you-accept-it).

### Choose how hard the task is

When the agent gives a task to a child, it chooses **low**, **normal**, or **high**. Go to **Settings → AI & Models → Sub-Agent Model Routing**. There you can choose one model for each level. Then easy tasks use a light model, and hard tasks use a strong model. If you do not choose, the child uses the same model as the parent.

### How much work at one time?

- There is a **limit** to the number of chats that work at the same time. This keeps your computer fast.
- If there is more work than the limit, the agent starts the work in groups. Because of this, parallel work can be slower than you expect.
- There is also a **safety limit** that stops a child from making more children without end. If a task goes over the limit, the app does not make a new child. It tells you.

---

## Isolation rules

Children use their **own work folder** by default. Parent-folder files do not follow automatically.

- When they must see the same deliverables, tell the agent to **reuse the current work folder**.
- Folder rules and project skills are read from **the folder the child uses**.
- Skills tied to **one assistant** follow the assistant chosen for the child.
- Skills for **your whole account** are available to parent and child.
- To point elsewhere, set a session **workspace override**.
- Asking to **reset the child’s conversation** clears chat and plans but keeps folder files. To keep browser tabs, continue without resetting.

If the child “assumes” parent-only files or rules, delegation fails. Put **goal, scope, output format, required paths** in the handoff, or **tell it to reuse the current work folder**.

Follow `@skill:delegate` for operating details.

---

## Which skill when?

Mention `@skill:name` or tell the agent the pattern name.

### 1) One-shot delegation · basics

| Skill          | When                                                                  |
| -------------- | --------------------------------------------------------------------- |
| **`delegate`** | Baseline: create / watch / nudge children and get results             |
| **`recruit`**  | Create a new **specialist assistant** (not session delegation itself) |
| **`boost`**    | Trim or strengthen tools on an **existing** assistant                 |

```
@skill:delegate
Read only the auth module in this repo and summarize vulnerability candidates in Markdown.
```

### 2) Work-splitting (ephemeral children)

| Skill                      | Pattern        | One line                                             |
| -------------------------- | -------------- | ---------------------------------------------------- |
| **`divide-conquer`**       | Parallel split | Non-overlapping pieces → merge                       |
| **`hub-spoke`**            | Hub–spoke      | Coordinator steers workers; no worker-to-worker chat |
| **`pipeline`**             | Sequential     | A output → B input → C …                             |
| **`consensus-delegation`** | Multi-view     | Same question to several experts → parent reconciles |
| **`gatekeeper`**           | Quality gate   | Writer ↔ reviewer loop                              |
| **`pair-programming`**     | Pair           | Two roles on a **shared work folder**                |

```
@skill:divide-conquer
Split frontend, backend, and docs review reports in parallel, then merge into one.
```

```
@skill:consensus-delegation
Review this PR from security, performance, and maintainability angles and reconcile opinions.
```

### 3) Persistent teams · Org

Different from one-off delegation. Use when you need a **team** and the **Org** screen.

| Skill                 | Role                                                                   |
| --------------------- | ---------------------------------------------------------------------- |
| **`teamwork`**        | Design the task force and roles — **prepare first**                    |
| **`org`**             | Create member sessions visible under Org. Resume from the **Org root** |
| **`org-restructure`** | Change roles or rules on an existing Org                               |

Flow: `teamwork` prepare → `org` create → continue from sidebar **Org** → change structure with `org-restructure`.

- **Org** ≠ scheduled tasks (`schedule`) ≠ in-session reminders (`loop`) ≠ wake-on-done (`call-me-back`)
- Plain parent–child delegation alone does **not** show under **Org**

```
@skill:teamwork
Create a research / implement / review task force and coordinate with hub-spoke.
```

---

## Patterns in detail

Each pattern below shows the steps that the bundled skill follows, in simple words. For more prompts that you can copy, read [Orchestration examples](orchestration-examples.md).

### `delegate`: give a task to one agent

**Use it when**: the task has a clear limit, and you want another agent to do it and give you the result. It is also good when the task needs a specialist assistant.

**Steps**

1. The agent decides if the task is good to give to another agent.
2. If an idle child with the same role exists, the agent uses it again. If not, the agent makes a new child.
3. The agent chooses **which folder** the child uses. By default, the child has its own folder. If the child must see the same files, choose "use the same folder".
4. The agent writes the goal, the limits, the result format, and the file locations. It does not leave anything out.
5. The agent checks progress and gives more instructions if needed. At the end, it merges the result.

**Example request**

```text
@skill:delegate
Read proposal.md in this folder as an investor.
- Use the same folder as this chat. Do not change the file.
- In your answer, write: 3 good points, 3 weak points, 5 questions that I must ask.
```

### `divide-conquer`: split the work and do it at the same time

**Use it when**: you have many tasks that do not depend on each other. Examples: translate 12 documents, or check each folder.

**Steps**: split → assign → start → watch → merge → check

- The pieces must **not overlap**. If two children change the same file, one will overwrite the other.
- If children use the same folder, give each child a **different file name**.
- After the merge, check the whole result. Ask again only for the pieces that have a problem.
- If a piece needs the result of an earlier piece, do not use this pattern. Use `pipeline`.

```text
@skill:divide-conquer
Translate the 12 Korean guides in the docs folder into English.
Make 3 groups of 4 and work on the groups at the same time.
Use glossary.md for the terms. At the end, check that no link is broken.
```

### `hub-spoke`: one leader directs the specialists

**Use it when**: the work has different jobs, such as research, analysis, and a first draft, and one agent must put them together.

**How it works**: The leader (the hub) makes the plan and divides the work. The workers (the spokes) report **only to the leader**. The workers do not talk to each other. If worker B needs the result of worker A, the leader sends a summary to B.

- Tell the leader to ask for **summaries and file lists**, not long chat records. If the records are too long, the leader cannot work.

```text
@skill:hub-spoke
Check if we should start to use a new payment service.
You are the leader. The workers are: 1 price research, 2 how to use it and how hard it is to start, 3 law and security check.
Get a one-page summary from each worker. Then make a comparison table and a recommendation.
```

### `pipeline`: do the work in steps

**Use it when**: the result of one step is the input of the next step. Example: research → first draft → review → final version.

- All steps use the **same folder**.
- The next step gets only a **summary and the file location**. It does not get the whole chat.
- If one step fails, only that step runs again.
- If the steps do not depend on each other, `divide-conquer` is faster.

```text
@skill:pipeline
Write a blog post with the title "Make a weekly report with a local agent".
Step 1: research, with at least 5 sources. Step 2: first draft. Step 3: check the facts and the tone. Step 4: final version.
Save the result of each step in the drafts folder.
```

### `consensus-delegation`: one question, many points of view

**Use it when**: you want to look at one decision from many sides. Examples: cost, risk, and ease of use.

**How it works**: Two to four children each look at **the same question in the same format**. They work alone. They do not see the answers of the other children. The parent compares the answers. The parent asks again **only about the points where the answers are different**. There is no vote. The parent looks at the proof and decides.

Each child answers in the same format.

```text
Result: agree | agree with conditions | disagree | cannot decide
Confidence: high | medium | low
What I found: - [point]: [proof]
Risks: - [point]: [why it is important]
```

Tip: do not make a new assistant for each point of view. In your request, write the role, for example "You are the cost reviewer". This is enough.

### `gatekeeper`: make, check, and repeat

**Use it when**: the result must pass fixed rules.

**How it works**: The writer makes the result. The reviewer checks it against the rules. The reviewer decides to **pass** or to **reject**. If the reviewer rejects it, the writer gets the list of problems, fixes them, and sends the result again. The loop stops after **3 rounds at most**. This stops the loop that never ends because of small differences.

```text
@skill:gatekeeper
Let the writer make a first draft of this month's notice.
The reviewer checks 3 rules: 1 no wrong facts, 2 the most important point is first, 3 each item has no more than two sentences.
If the notice does not pass in 3 rounds, tell me what is still wrong.
```

### `pair-programming`: two agents take turns

**Use it when**: you want two agents to change the same file in turns and check each other. You can use it for code and for long documents.

**How it works**: Two children work in the **same folder**. One child (the driver) makes the change and writes what it changed. The other child (the navigator) reads the change and shows the problems. The parent gives the turn to each child. When the work goes to the next stage, the children **change roles**.

```text
@skill:pair-programming
Improve the setup section in user-guide.md together.
One agent changes the text. The other agent reads the changes and finds what is wrong or not clear.
When half of the work is done, change the roles.
```

---

## Check the result before you accept it

For important work, add **`delegation-eval-loop`** after `delegate`. **The child makes the result. The parent decides if it is good.** A child that says "I am done" is not enough.

**Steps**

1. **Write the pass rules first.** Write the goal, the places that the child can change, the places that the child must not change, a checklist (`- [ ] no spelling errors`), and where the result goes.
2. **Check four things in order.** If a check fails, send the work back at once.

   | Order | What the parent checks | In simple words                                                                     |
   | ----- | ---------------------- | ----------------------------------------------------------------------------------- |
   | 1     | The fixed checks       | The parent runs the same checks again                                               |
   | 2     | The limits             | The parent looks if the child changed a place that it must not change               |
   | 3     | If the work is real    | The parent finds "done" with no proof, work that stopped, and work that did not run |
   | 4     | If it is what you want | After checks 1 to 3 pass, the parent makes the last decision                        |

3. **Be exact when you send work back.** Do not write "do it again". Send the exact problem.
4. **Set a limit.** After 3 to 5 rounds, the agent stops. It tells you the reason.

```text
@skill:delegate @skill:delegation-eval-loop
Change all dates in the customer notices to the format "March 5, 2026".
- Places you can change: the notice folder. Do not change other places.
- Pass rules: - [ ] all dates have the same format  - [ ] numbers that are not dates stay the same
- In your answer, write the list of changed files and 3 examples of before and after.
```

---

## Features that look the same but are not

These features are **not** delegation and are **not** Org. Choose the one that you need.

| What you need                                                  | Skill              | Description                                                                                        |
| -------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------------------------------- |
| "Remind me in 5 minutes", "check every morning" (in this chat) | `loop`             | The agent comes back to this chat at the set time                                                  |
| "Tell me when the build is done", a webhook, a ticket status   | `call-me-back`     | The agent waits for a **done signal**, not for a time. Read the [guide](call-me-back.md)           |
| Run on a plan, also when the app is closed                     | `schedule`         | It runs in the whole app and does not depend on a chat. Read [Scheduled Tasks](scheduled-tasks.md) |
| A team that works for a long time                              | `teamwork` → `org` | The team shows in the **Org** view. You continue from the leader chat of the team                  |

---

## Steps to run a long-term team (Org)

1. **`teamwork`**: First, the agent decides if the work really has many tracks. If one agent is enough, it does not make a team. Then it sets the goal, the roles, and the way of work. It puts the team rule files (`MISSION.md`, `ROLES.md`, and others) in a folder that only the app uses. The way of work is one of these: **in order (Sequential)**, **leader at the center (hub-and-spoke)**, or **free work from a shared board (Swarm)**.
2. **`org`**: The agent makes the Org **one time** from the leader chat. It makes the team members as part of the Org. Later, you continue from the **leader chat of the Org**. If you open the chat of one member, this is not the same as opening the Org.
3. **`org-restructure`**: The agent adds, removes, or joins roles, and changes rules. It does not remove the team.
4. **`postmortem-improve`**: When the work is done, or the same failure happens again, the agent writes what it learned. Then it makes the team better.

You can move an Org to another computer. Read [export and import](#where-it-shows-in-the-ui).

---

## Quick picker

| Goal                                | Use                                |
| ----------------------------------- | ---------------------------------- |
| One child, get the result           | `@skill:delegate`                  |
| Parallel independent pieces         | `@skill:divide-conquer`            |
| Central coordinator + workers       | `@skill:hub-spoke`                 |
| Ordered stages                      | `@skill:pipeline`                  |
| Same topic, many viewpoints         | `@skill:consensus-delegation`      |
| Write ↔ strict review loop         | `@skill:gatekeeper`                |
| Two agents pair on code             | `@skill:pair-programming`          |
| Long-lived team + Org UI            | `@skill:teamwork` → `@skill:org`   |
| Hire a new specialist assistant     | `@skill:recruit` (then `delegate`) |
| Slim tools on an existing assistant | `@skill:boost`                     |

Who a skill applies to: [Skills](skills.md).

---

## Tips

1. **Smallest structure first** — do not build a team if one agent is enough.
2. **Self-contained handoffs** — children should not guess parent chat.
3. **Keep the parent free during parallel work** — follow the skill’s async handoff guidance.
4. **Verify child output** before the final answer.
5. **On delete** — choose whether to include sub-agents in History.

---

## Related

- [Skills](skills.md)
- [First agent chat](../getting-started/first-agent.md)
- [Troubleshooting](troubleshooting.md)
