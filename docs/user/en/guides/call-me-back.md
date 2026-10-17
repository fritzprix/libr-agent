---
title: Continue when a job is done (call-me-back)
description: 'Ask LibrAgent to wait for a build, a script, a webhook, or an outside tool, and to continue in the same chat when the job is done.'
---

# Continue when a job is done

> Use `@skill:call-me-back` when you want the agent to **wait for a job** and then **continue in the same chat**.  
> The agent does not guess. It waits for a real "done" signal.

---

## What this skill does

The LibrAgent principle is: **check, do not guess**. The agent does not guess that a job will end in a few minutes. It waits for a real signal that the job is done. All work stays on your computer.

Some jobs take a long time. Examples: a build, a test run, a data export, or a ticket that a person must close.

You do not need to watch these jobs. Tell the agent what to wait for. When the job is done, the agent continues in the **same chat** and uses the result.

"Me" in "call me back" means **this chat session**. The skill does not send a phone call or a text message.

---

## Which skill do I use?

| You want to...                                            | Use            | Example                              |
| --------------------------------------------------------- | -------------- | ------------------------------------ |
| Wait for a job to be **done**                             | `call-me-back` | "Tell me when the build is done."    |
| Come back at a **time**, in this chat                     | `loop`         | "Check again in 30 minutes."         |
| Run a task on a **schedule**, even when the app is closed | `schedule`     | "Run a report every Monday at 9."    |
| Give work to a **second agent** now                       | `delegate`     | "Ask a reviewer to check this file." |

Rule: if you name a **job, a command, or a tool**, use `call-me-back`. If you name only a **time**, use `loop`.

---

## Case 1: A job on your own computer

This is the easy case. The agent starts the job and waits for it. You do not need to set up anything.

```text
@skill:call-me-back
Run the full test suite in the background.
When it is done, tell me which tests failed and why.
```

What happens:

1. The agent starts the job in the background.
2. The agent waits until the job stops. It does **not** check again and again.
3. The agent reads the output of the job.
4. The agent tells you the result in this chat.

> **Note**: You can keep the chat open or work in another page. The agent continues when the job stops.

---

## Case 2: A tool outside LibrAgent

Use this case for tools such as a webhook, a file watcher, or a build server. These tools must **send a message to this chat** when they are done.

```text
@skill:call-me-back
My build server runs the deploy job.
Set up the build server to send a message to this chat when the job is done.
Then wait, and tell me if the deploy worked.
```

What happens:

1. The agent finds the **address of this chat**. The address is only for this chat. It works only on your own computer.
2. The agent sets up the outside tool to send a short message to that address when the job is done.
3. The agent waits.
4. The outside tool sends the message. The agent continues the work in this chat.

### The message must reach the chat

The setup is **not complete** until the outside tool sends a message to the chat address.

A tool that only prints text or writes a file does **not** continue the chat. The agent cannot see that text.

> **Warning**: Do not use a callback that only prints text, or only saves a file. The chat will wait and nothing will happen.

### Safety

- The chat address works only on **your own computer**. Other computers cannot use it.
- The address is different for each session. Do not share it.
- A strange web page on the internet cannot send a message to this address.

---

## What to write in your request

A good request has three parts.

| Part                 | Example                                                      |
| -------------------- | ------------------------------------------------------------ |
| **What to wait for** | "the `deploy` job on the build server"                       |
| **What to do after** | "tell me if it worked and show the last 20 lines of the log" |
| **What if it fails** | "if it failed, find the cause and suggest a fix"             |

---

## Common mistakes

| Problem                                           | Cause                                                             | Fix                                                                      |
| ------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------ |
| The agent says "I will check again in 5 minutes"  | The agent used `loop` (time) and not `call-me-back` (done signal) | Name the job. Say "wait until the job is done".                          |
| Nothing happens after the outside job is done     | The outside tool did not send a message to the chat               | Ask the agent to check that the tool sends a message to the chat address |
| The outside tool shows an error about the address | The address is wrong, or the app was restarted                    | Ask the agent for the current address and set up the tool again          |
| The agent wants to wait for a long time           | The job is very long                                              | Use `schedule` if the job must run when the app is closed                |
| You only wanted a reminder                        | A reminder is time-based                                          | Use `loop`: "Remind me in 10 minutes."                                   |

---

## Related

- [Automation overview](automation.md)
- [Sub-agents & orchestration](sub-agents.md)
- [Scheduled Tasks](scheduled-tasks.md)
- [Orchestration examples](orchestration-examples.md)
