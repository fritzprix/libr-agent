# Hero Demo Spec

Canonical **60-second product story** for README, homepage, release posts, Show HN, and social clips.

This replaces the retired `assets/demo_1280_4x_optimized.gif` claim (outdated UI + swarm caption without a finish line). Do **not** ship a new GIF until it matches this spec.

---

## Positioning line (must appear)

| Lang | Line |
| ---- | ---- |
| EN | **LibrAgent is not a chat app. It is an execution environment for agents.** |
| KO | **LibrAgent는 채팅 앱이 아닙니다. 에이전트를 위한 실행 환경입니다.** |
| ZH | **LibrAgent 不是聊天应用，而是代理的执行环境。** |

Supporting line:

| Lang | Line |
| ---- | ---- |
| EN | **One session. One goal. One deliverable on your machine.** |
| KO | **한 세션. 한 목표. 내 머신에 남는 산출물 하나.** |
| ZH | **一次会话。一个目标。在你机器上留下一份可保存的交付物。** |

README hero sections (all locales) must use the same story; translate wording, do not change the beat.

---

## Hero story (single narrative)

| Field | Value |
| ----- | ----- |
| Title | From “I need this done” to a finished deliverable — on your machine |
| Persona | Solo developer / power user |
| Setting | Fresh LibrAgent install, local or API model already connected |
| Goal | Review a small local repo and leave a **markdown report file** the user keeps |
| Proof | Viewer sees: connect project → agent reads/runs → browser or file proof → deliverable card/file |
| Explicitly out of scope | Swarm, org, Settings tour, multi-assistant recruiting, feature laundry list |

Secondary demos (separate clips, never the hero):

1. **Morning Briefing** — schedule → file appears next morning
2. **Logged-in browse** — Settings → **Saved browser logins** → import → `use_profile` → authenticated page (after #1952 lands)

---

## 60-second shot list

| Time | On screen | EN | KO | ZH |
| ---- | --------- | -- | -- | -- |
| 0–5s | Chat with model badge visible (Ollama or API) | Local agent. Your machine. | 로컬 에이전트. 내 머신. | 本地代理。你的机器。 |
| 5–15s | Attach / open Workspace on a real folder | Connect a real project. | 실제 프로젝트를 연결하세요. | 连接真实项目。 |
| 15–30s | Tool calls: read file, optional shell/test | It reads and runs — not just suggests. | 읽고 실행합니다 — 제안만 하지 않습니다. | 会读也会跑——不只是建议。 |
| 30–45s | Browser open **or** edited file diff (pick one, not both) | It can leave the editor. | 에디터 밖에서도 일합니다. | 不只停在编辑器里。 |
| 45–55s | `reportResult` / deliverable / saved `.md` in workspace | Work finishes as a file you keep. | 결과는 남는 파일로 끝납니다. | 工作以你能保存的文件收尾。 |
| 55–60s | Releases / Download affordance | Install. Run locally. | 설치하고, 로컬에서 실행. | 安装。在本地运行。 |

Rules:

- One chat session only. No tab-hopping the whole app.
- No menu walkthrough. No “and also swarm…”.
- Prefer **1080p WebM + GIF fallback**; keep under ~15MB for README if possible.
- Burn in **EN / KO / ZH** from this table for localized posts; do not invent a different story per language.

---

## Filming script (operator checklist)

### Prep (5 min)

1. Use a **throwaway demo repo** (e.g. `demo-todo` with 3–5 files + one intentional smell).
2. Clean LibrAgent profile or a dedicated demo user-data so UI chrome is calm.
3. Model ready (fast enough for recording; hide rate-limit toasts).
4. Window: 1280×720 or 1920×1080; OS theme light or dark — pick one and stick to it.
5. Disable unrelated notifications; hide secrets in Settings before recording.

### Prompt to type on camera (copy-paste)

```text
You are helping me ship. In this workspace:

1. Skim the project structure.
2. Find the most important risk or bug for a new contributor.
3. Write a short report to `DELIVERABLE.md` in the workspace root with:
   - Summary (3 bullets)
   - Evidence (file paths)
   - Recommended next step
4. When done, use reportResult so I can open the file from chat.

Do not ask clarifying questions — make reasonable assumptions and finish.
```

### Optional browser beat (swap into 30–45s)

Only if Workspace-only feels thin:

```text
After writing DELIVERABLE.md, open the project's README in the built-in browser
(or the local docs URL if present) and confirm the title matches the repo name.
Mention that check in the report.
```

### Capture

1. Record once clean; avoid mouse thrashing.
2. Export WebM (primary) + optimized GIF (optional README embed).
3. Burn in EN (or KO/ZH) subtitles from the shot list (or hard-code in CapCut/OBS).

### Acceptance criteria

- [ ] Viewer can retell the story in one sentence without reading README
- [ ] Deliverable file is visibly opened or highlighted at the end
- [ ] No swarm / org / pin-playbook footage in the hero clip
- [ ] Filename for assets: `assets/hero-demo-60s.webm` (+ optional `.gif`)
- [ ] README hero section embeds the new asset above the text story (no process notes in the public README)

---

## Release / channel copy (reuse)

**Show HN / Reddit title**

> LibrAgent – local-first desktop execution environment for AI agents (MCP, workspace, browser)

**First comment / post body (short)**

LibrAgent is not another chat shell. In one local session an agent can attach your project, read and run tools, and leave a file you keep. MCP-native, Tauri desktop, your machine.

Download: https://github.com/fritzprix/libr-agent/releases/latest

**Release notes opener (template)**

> This release advances the hero story: _one session → real tools → a deliverable on your machine._  
> (Then list only features that strengthen that beat.)

---

## Ownership

- Spec owners: product messaging + release captain
- Do not merge README demo media that diverges from this file without updating the spec first
