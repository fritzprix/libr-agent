# Feature Tips (Spotlight) — Release Update Guide

During each LibrAgent release, decide whether layman-facing changes deserve an
in-app tip. Tips live in the spotlight module and surface as:

- **Release Spotlight** — What's New card on the chat hub after a version bump
- **Hub Hint** — rotating tip card on the chat hub
- **Wait tip** — one-line tip above Chat Input (`ComposerBusyTipStrip`) while
  `workflowStatus === 'busy'`, rotating every ~10s

## When to add or update a tip

Add/update a tip **only** if a change is something a non-developer end user
should discover without reading CHANGELOG.

| Include (layman) | Skip |
|------------------|------|
| New capability the user can try (browser profile import, scheduled starters, charts, Knowledge graph, themes) | Pure refactors, type cleanup, test-only |
| New Settings control with visible UI | Internal harness / CI / docs-only |
| UX that changes day-to-day chat/hub behavior | Bugfix that restores expected behavior (unless the feature itself was previously invisible) |
| New entry point / page / workflow | Dependency bumps, logging, telemetry |

**Cap per release:** at most **1–3** new or refreshed tips. Prefer quality over volume.

If nothing qualifies, write `No tip updates this release` in the release notes
commit body / checklist and move on (do not invent filler tips).

## Files to touch

| Purpose | Path |
|---------|------|
| Tip registry | `src/features/spotlight/feature-spotlights.ts` |
| i18n (required for all 8 locales) | `src/locales/{en,ko,ja,zh,de,fr,es,pt}/common.json` → `spotlight.items.<camelId>.*` |
| Settings copy (only if toggle wording changes) | `settings.display.showFeatureTips*` in the same locale files |

Do **not** invent fake CTAs. `href` must open a real Settings tab or route.

## Tip shape (registry)

```typescript
{
  id: 'kebab-case-stable-id',       // SSOT; never rename casually
  sinceVersion: '<NEW_VERSION>',    // e.g. "0.9.22" — this release
  titleKey: 'spotlight.items.<id>.title',
  bodyKey: 'spotlight.items.<id>.body',
  ctaLabelKey: 'spotlight.items.<id>.cta',
  href:
    | { type: 'settings'; tab: 'general' | 'system' | 'advanced' | 'ai-models' }
    | { type: 'route'; path: '/scheduled-tasks' /* etc. */ },
  surfaces: ['release', 'hub'] | ['release', 'hub', 'wait'] | ['hub'],
  priority: number,                 // lower = higher priority (1 is first)
}
```

### Surfaces rules

- Always include **`release`** for tips that should appear in What's New after upgrade.
- Include **`hub`** for ongoing discovery after What's New is dismissed.
- Include **`wait`** only when CTA is safe mid-session:
  - Prefer Settings tabs or side pages (`/knowledge`, `/scheduled-tasks`).
  - **Do not** put `wait` on tips that navigate to `/agent/draft` or leave the active session.

### Priority

- New release tips: assign priority **1–3** (ahead of older evergreen tips).
- Bump older tips' priority numbers down if needed so the pool stays ordered.
- Evergreen low-value tips (e.g. themes) stay hub-only with high priority numbers.

### i18n checklist

For each new tip id `fooBar` (camelCase under `spotlight.items`):

1. Add `title`, `body`, `cta` under `spotlight.items.fooBar` in **all 8** locales.
2. Keep layman tone: short title, one-sentence body, action CTA.
3. Product/UI names (`System`, `Scheduled Tasks`, `Thinking Effort`) may stay in English when that matches the UI label.

## Procedure during release-manager Step 2

1. From the drafted CHANGELOG section, list candidate **layman** bullets.
2. For each candidate, decide: new tip / refresh existing tip / skip.
3. If new:
   - Append entry to `FEATURE_SPOTLIGHTS` with `sinceVersion = <planned NEW_VERSION>`.
   - Add i18n keys (all locales).
4. If refresh (same feature, better wording or CTA):
   - Update copy in locales; set `sinceVersion` to `<NEW_VERSION>` so Release Spotlight picks it up again for upgraders.
5. Sanity-check:
   - No `wait` + session-leaving CTA.
   - Registry `titleKey` / `bodyKey` / `ctaLabelKey` match locale paths.
6. Include tip file + locale changes in the docs/changelog commit (or a sibling `feat(spotlight): …` commit before the release script).

## Example mapping

| CHANGELOG (user-facing) | Tip action |
|-------------------------|------------|
| “Import Chrome profiles in Settings → System” | New/update `browser-profile-import`, surfaces `release+hub+wait`, href settings/system |
| “CSV chart from chat drop” | Update `visualize-csv` for hub/release only (no wait) |
| “Fixed race in tool loop” | Skip tip |
| “Bump rmcp” | Skip tip |
