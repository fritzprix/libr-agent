---
title: Orchestration examples
description: 'Prompts that you can copy, for tasks from one helper to a long-term team. Each example uses everyday work and shows common mistakes.'
---

# Orchestration examples

> How to use the patterns in [Sub-agents & orchestration](sub-agents.md) for everyday work.  
> Copy a request. Change the details.

## First, use only what you need

If one agent is enough, **do not hand off the work**. Make the structure bigger only when the work has many parts, when you need another point of view, or when the work continues for a long time. LibrAgent starts small. It checks a result before it uses it.

| Situation                                            | Skill                                |
| ---------------------------------------------------- | ------------------------------------ |
| Give one block of work to another agent              | `delegate`                           |
| Many tasks that do not depend on each other          | `divide-conquer`                     |
| The result of one step is the input of the next step | `pipeline`                           |
| One question, many points of view                    | `consensus-delegation`               |
| One leader directs the specialists                   | `hub-spoke`                          |
| Make, check, and repeat                              | `gatekeeper` ([loop guide](loop.md)) |
| A long-term team and the Org view                    | `teamwork` → `org`                   |

---

## Example 1. Get a review from a different point of view (`delegate`)

**Use it when**: you want another agent to read your document.

```text
@skill:delegate
Read proposal.md in this folder as an investor.
- Use the same folder as this chat. Do not change the file.
- In your answer, write: 3 good points, 3 weak points, 5 questions that I must ask.
```

**What happens**: The agent finds a reviewer child, or makes a new one. The child reads the document and gives the result in its answer. The parent checks that the child stayed in the limits. Then the parent gives you the result.

**Watch out**: The child can say "I cannot find the file". This means the child has its own folder. Write "Use the same folder as this chat". Or paste the text into your request and ask again. If you asked for a review by a different agent, and the parent judged the work itself, ask again.

---

## Example 2. Do many parts at the same time (`divide-conquer`)

**Use it when**: you have tasks that do not depend on each other.

```text
@skill:divide-conquer
Write this quarter's report in three parts at the same time.
1 Sales summary. 2 Customer feedback. 3 Plan for the next quarter.
Save each part in the report folder. At the end, join them into one file named SUMMARY.md.
```

**What happens**: The agent splits the work into three parts that do not overlap. It gives each part a different file name. Three children work at the same time. The parent prepares the join. At the end, the parent joins the parts and asks again only for the weak parts.

**Watch out**: If the parts use the same folder, use **different file names**. Ask the agent to check that the summary agrees with each part.

---

## Example 3. Write a post in steps (`pipeline`)

**Use it when**: the result of one step is the input of the next step.

```text
@skill:pipeline
Write a blog post with the title "Make a weekly report with a local agent".
1) Research: at least 5 sources. Save in drafts/01-research.md
2) First draft: use the research. Save in drafts/02-draft.md
3) Review: check facts and tone. Write the changes in drafts/03-review.md
4) Final version: use the review. Save in final.md
```

**What happens**: All steps use the same folder. The next step gets only a summary and the file location, not the whole chat. If the review finds a problem, only the draft step runs again.

**Watch out**: If the steps do not depend on each other, `divide-conquer` is faster than `pipeline`.

---

## Example 4. Look at one decision from many sides (`consensus-delegation`)

**Use it when**: you must make an important decision, for example to change to a new service. You want to look at cost, risk, and ease of use.

```text
@skill:consensus-delegation
Check if we should change our team messenger to a new service.
Look at three sides, each alone: cost, risk (security and the work to move), ease of use.
Each side answers in the same format: result, confidence, what it found, risks.
Ask again only about the points where the answers are different.
Then give a final advice: change, change with conditions, or keep.
```

**Example result**

| Side        | Result                | Main proof                                             |
| ----------- | --------------------- | ------------------------------------------------------ |
| Cost        | Agree with conditions | The yearly cost is lower, but the move costs much work |
| Risk        | Agree with conditions | A security check is needed when you move the data      |
| Ease of use | Disagree              | Half of the team does not know the new screens         |

The parent asks again **only about the points that conflict**. Here, the conflict is good cost against bad ease of use. The final advice has the proof for each point.

**Watch out**: Write the role in your request. You do not need to make a new assistant. Two to four sides is enough.

---

## Example 5. A leader directs the specialists (`hub-spoke`)

**Use it when**: the work has different jobs, such as research, analysis, and a summary, and one agent must put them together.

```text
@skill:hub-spoke
Make a comparison report for three competing services (A, B, C).
You are the leader. The workers are: 1 compare the functions, 2 compare the prices, 3 collect what users say.
Each worker gives only the leader a one-page summary and the source links.
The leader gets only the summaries. Then the leader writes a comparison table and "3 things that we can learn".
```

**What happens**: The leader makes a task list. The workers work at the same time. If worker 3 needs the result of worker 2, the leader sends a summary. The workers do not talk to each other.

**Watch out**: If the leader reads the long chat records of all workers, it cannot work. Write "Get only summaries and file lists".

---

## Example 6. Make, check, and make again (`gatekeeper`)

**Use it when**: a notice must follow fixed rules.

```text
@skill:gatekeeper
Let the writer make this month's customer notice.
The reviewer checks 3 rules: 1 no wrong facts, 2 the most important point is first, 3 each item has no more than two sentences.
If the notice does not pass in 3 rounds, tell me what is still wrong.
```

**What happens**: The reviewer uses the rules to pass or reject the notice. If the reviewer rejects it, the writer gets the list of problems. The loop runs 3 times at most. If the notice still does not pass, the agent tells you the reason.

**Watch out**: Make the rules **exact**. A rule such as "write it well" gives a loop that does not end.  
For clock wakes that run writer and reviewer on a task list, see [Session loops (loop)](loop.md).

---

## Example 7. Check the result before you accept it (`delegate` + `delegation-eval-loop`)

**Use it when**: you need a **checked result**, not only the words "I am done".

```text
@skill:delegate @skill:delegation-eval-loop
Change all dates in the customer notices to the format "March 5, 2026".
- Places you can change: the notice folder. Do not change other places.
- Pass rules: - [ ] all dates have the same format  - [ ] numbers that are not dates stay the same
- In your answer, write the list of changed files and 3 examples of before and after.
```

**What happens**: When the child is done, the parent checks the pass rules itself. The parent also looks if the child changed a place that it must not change. If there is a problem, the parent sends the work back with the exact problem. After 3 to 5 rounds, the agent stops and tells you the reason.

**Watch out**: "No problem" from the child is not proof. A run that stopped, or "done" with no proof, is a failure.

---

## Example 8. Make a long-term team (`teamwork` → `org`)

**Use it when**: the same team works for many weeks, and you want to see its state in one place.

```text
@skill:teamwork
Make a team for a "monthly market report".
The roles are: research, data analysis, and editor. A leader should direct the work.
Save the result as a file for each month in the reports folder. The editor gives the last approval.
```

When the team is ready, write:

```text
@skill:org
Make the Org from the team that you just prepared. Start with the research role.
```

**What happens**: The agent writes the goal, the roles, and the work rules, and saves them as the team rules. Then it makes the Org and adds the team members. The team shows in the **Org** view in the sidebar. Next month, you continue from the **leader chat of the Org**.

To change the team later:

```text
@skill:org-restructure
Join the data analysis role with the research role. Add the approval rules of the editor to the role description.
```

**Watch out**: A task that you hand off one time does not show in Org. For work that repeats at set times, use [Scheduled Tasks](scheduled-tasks.md), not Org. To move a team to another computer, use **Export Org** and **Import Org** in the Org view.

---

## Example 9. Wait for something to finish (`call-me-back` / `loop`)

Here you do not hand off work. You need the agent to **wait**.

```text
@skill:call-me-back
Start the job that converts all pictures in this folder. Run it in the background.
When it is done, continue in this chat and tell me if some files failed.
```

```text
@skill:loop
Check again in 30 minutes if this job is done.
```

- Wait until a job is **done**: `call-me-back` ([guide](call-me-back.md))
- Look again after some **time**: `loop` ([Session loops](loop.md)). Use `schedule` if the work must run when the app is closed.

---

## Common mistakes and fixes

| Problem                                           | Reason                                                              | Fix                                                                           |
| ------------------------------------------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| The child cannot find a file                      | The child has its own work folder                                   | Write "Use the same folder as this chat", or paste the text into your request |
| The parent cannot read the result of the child    | The child only saved a file                                         | Write "Put the result in your answer"                                         |
| Children overwrite the files of each other        | They use the same file name                                         | Give each child a different file name or subfolder                            |
| New children keep appearing                       | The agent does not use idle children again                          | Write "Use an idle child with the same role again"                            |
| A message says the app cannot make a new child    | The safety limit is reached                                         | Do not let children make more children. Make the structure simpler            |
| Parallel work is slow                             | There is a limit for the number of chats that work at the same time | Use smaller groups, or split the work into steps                              |
| The check never ends                              | The pass rules are not exact                                        | Write a checklist and a maximum number of rounds                              |
| A task that you handed off one time is not in Org | This is correct                                                     | Use `teamwork` → `org` if you need an Org                                     |

For more help, read [Troubleshooting](troubleshooting.md).

---

## Related

- [Sub-agents & orchestration](sub-agents.md)
- [Skills](skills.md)
- [Scheduled Tasks](scheduled-tasks.md)
- [Automation overview](automation.md)
