# Hero shot list (condensed)

Canonical full spec: repo `docs/contributing/hero-demo-spec.md`.

## Positioning (must appear on film)

- EN: **Install tools like apps. Keep your model. Keep the file.**
- KO: **툴은 앱처럼 설치하고, 모델은 내가 고르고, 결과는 파일로 남깁니다.**

## Primary beat (~70s)

| Time | On screen | Demo-play action |
| ---- | --------- | ---------------- |
| 0–5s | Chat + model badge | Operator / already on agent |
| 5–22s | Extensions → Install | `play_hero_chrome.sh hn` (quiet preset only; **not** `serena`) |
| 22–35s | Workspace attached | Operator or session with `workspacePath` |
| 35–50s | Tool calls | Session `request` / messages |
| 50–65s | `DELIVERABLE.md` / reportResult | Deliverable prompt below |
| 65–70s | Releases | Operator |

## Deliverable prompt (after Install + workspace)

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

## Secondary clips (separate; never replace hero)

1. `@skill:pipeline` (or hub-spoke / divide-conquer) → one merged report
2. Morning Briefing recipe Install → Run now
3. Import MCP from Cursor
4. Sticky agent browser profile → authenticated page (sign in once; clear via Settings)

## Filming tips

- 1280×720 or 1920×1080; one theme
- Preset **not** already installed if Install must be visible (uninstall a
  quiet preset first — never use `serena`, it opens a browser dashboard)
- Linger ~2s on Install success
- No org/swarm laundry list on screen
