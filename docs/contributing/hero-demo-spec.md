# Hero Demo Spec

Canonical **product demo** for README, homepage, release posts, Show HN, and social clips.

This replaces the retired `assets/demo_1280_4x_optimized.gif` claim (outdated UI + swarm caption without a finish line). Do **not** ship a new GIF/WebM until it matches this spec.

Product positioning locked with [product-messaging-guide.md](./product-messaging-guide.md) and the main [README.md](../../README.md):

1. **Surface without harness homework** (Extensions one-click, GUI)
2. **Orchestration as product** (secondary clip — named `@skill:` patterns)
3. **Provider & stack freedom** (model badge; optional Cursor import clip)

---

## Positioning line (must appear)

| Lang | Line |
| ---- | ---- |
| EN | **Install tools like apps. Keep your model. Keep the file.** |
| KO | **툴은 앱처럼 설치하고, 모델은 내가 고르고, 결과는 파일로 남깁니다.** |
| ZH | **像装应用一样安装工具。模型自选。结果落成文件。** |

Supporting line (optional burn-in):

| Lang | Line |
| ---- | ---- |
| EN | **No vendor harness. No JSON homework.** |
| KO | **벤더 하네스 없음. JSON 숙제 없음.** |
| ZH | **不绑厂商。不写 JSON 作业。** |

Do **not** lead the hero with “not a chat app / execution harness” alone — that is category noise shared with every coding agent.

README hero sections (all locales) must use the same story; translate wording, do not change the beat.

---

## Hero story (primary — ~70s)

| Field | Value |
| ----- | ----- |
| Title | From empty tooling to a file you keep — without editing MCP JSON |
| Persona | Operator / power user (not “senior who loves shell configs”) |
| Setting | Fresh LibrAgent install; model already connected (API **or** Ollama badge visible) |
| Goal | One-click install a preset → attach workspace → leave a **markdown deliverable** |
| Proof | Extensions **Install** → tool use → `DELIVERABLE.md` / reportResult |
| Explicitly out of scope | Org/swarm laundry list, Settings tours, Harbor/ATIF, sudo deep-dives |

### Secondary demos (separate clips, never replace the hero)

1. **Pick a pattern** — `@skill:pipeline` (or hub-spoke / divide-conquer) → staged work → one merged report. Proves orchestration-as-product.
2. **Morning Briefing recipe** — recipe Install → presets + assistant + schedule → Run now → briefing file.
3. **Import from Cursor** — agent imports MCP → Extensions list fills (acquisition / freedom).
4. **Logged-in browse** — Agent sticky browser profile: sign in once inside LibrAgent’s agent browser, then later sessions reuse those cookies until cleared in Settings → System → Agent browser.

---

## Shot list (primary hero)

| Time | On screen | EN | KO | ZH |
| ---- | --------- | -- | -- | -- |
| 0–5s | Chat with model badge (Ollama **or** API) | Your model. | 내 모델. | 你的模型。 |
| 5–22s | **Extensions** → pick GitHub (or similar) preset → **Install** succeeds | Install tools like apps. | 툴은 앱처럼 설치. | 像装应用一样装工具。 |
| 22–35s | Attach / open Workspace on a real folder | Connect a real project. | 실제 프로젝트 연결. | 连接真实项目。 |
| 35–50s | Tool calls: read / optional shell (calm UI) | It runs — not config theater. | 실행합니다 — 설정 쇼가 아닙니다. | 真跑——不是配置表演。 |
| 50–65s | `reportResult` / `DELIVERABLE.md` opened or highlighted | Work finishes as a file you keep. | 결과는 남는 파일. | 工作以文件收尾。 |
| 65–70s | Releases / Download | Install LibrAgent. | LibrAgent 설치. | 安装 LibrAgent。 |

Rules:

- Prefer **one** main chrome path: Extensions → Chat/Workspace. Avoid tab-hopping the whole app.
- Do **not** list swarm/org/skills catalogs on screen.
- Prefer **1080p WebM + GIF fallback**; keep under ~15MB for README if possible.
- Burn in **EN / KO / ZH** from this table for localized posts; do not invent a different story per language.

---

## Filming script (operator checklist)

### Prep (5 min)

1. Use a **throwaway demo repo** (e.g. `demo-todo` with 3–5 files + one intentional smell).
2. Clean LibrAgent profile or a dedicated demo user-data so UI chrome is calm.
3. Model ready (fast enough for recording; hide rate-limit toasts). Show **either** Ollama or API badge deliberately (freedom beat).
4. Window: 1280×720 or 1920×1080; OS theme light or dark — pick one and stick to it.
5. Disable unrelated notifications; hide secrets in Settings before recording.
6. Confirm the GitHub (or chosen) preset is **not** already installed so Install is visible.

### Prompt to type on camera (after Install + workspace)

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

### Optional: secondary “pick a pattern” prompt (Demo B only)

```text
@skill:pipeline
Research this workspace for contributor risks, draft a short report, then review it.
Leave one final DELIVERABLE.md and call reportResult. Do not ask clarifying questions.
```

### Capture

1. Record once clean; linger on the Extensions **Install** success state (~2s).
2. Export WebM (primary) + optimized GIF (optional README embed).
3. Burn in EN (or KO/ZH) subtitles from the shot list.

### Optional: remote chrome drive (App Control MCP)

For Cursor / CI / hands-free filming, drive the UI with **generic** primitives on `POST /mcp/control` (not a demo-only composite tool). See [app-control-mcp.md](../features/app-control-mcp.md) and bundled skill **`demo-play`** (`@skill:demo-play`).

Enable: `--mcp --app-control` (or `LIBRAGENT_MCP_ENABLE` + `LIBRAGENT_APP_CONTROL`).

Example beat for shot 5–22s:

```bash
bash src-tauri/bundled_skills/demo-play/scripts/play_hero_chrome.sh hn
```

Quiet presets only (`hn`, `arxiv`, `ddg-search`, …). **Never `serena`** — it opens a local dashboard browser and breaks the take.

### Acceptance criteria

- [ ] Viewer can retell: *one-click tools → real project → file on disk* (and optionally *my model*)
- [ ] Extensions Install is unmistakable (not cut away mid-click)
- [ ] Deliverable file is visibly opened or highlighted at the end
- [ ] No org/swarm/feature laundry list in the hero clip
- [x] Filename for assets: `assets/hero-demo-60s.webm` (+ optional `.gif`)
- [x] README hero embeds the new asset above the text story (no process notes in the public README)

---

## Release / channel copy (reuse)

**Show HN / Reddit title**

> LibrAgent – desktop agent environment: one-click MCP, named orchestration skills, any model

**First comment / post body (short)**

Most agent harnesses expect JSON, a terminal, and either a vendor lock-in or a framework you assemble. LibrAgent is a desktop product: install MCP presets like apps, pick coordination patterns with `@skill:pipeline` / `hub-spoke` / …, bring your own model (API or Ollama), and finish with a file on your machine.

Download: https://github.com/fritzprix/libr-agent/releases/latest

**Release notes opener (template)**

> This release advances the product story: _one-click tools → your model → a deliverable you keep_ (and orchestration skills when one agent is not enough).  
> (Then list only features that strengthen those beats.)

---

## Ownership

- Spec owners: product messaging + release captain
- Do not merge README demo media that diverges from this file without updating the spec first
