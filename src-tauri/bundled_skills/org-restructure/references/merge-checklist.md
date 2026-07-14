# Merge Checklist

Combine two or more overlapping roles into one canonical role.

## When to use

- Duplicate specialists with fuzzy boundaries
- User says "merge marketing and product into one strategist"
- Two ROLES.md sections own the same artifacts

## Steps

### 1. Record the decision

```markdown
## Decision: Role merge
- Date:
- Merged: <Role A> + <Role B> → <Canonical Role>
- Canonical agent ID: <uuid from ROLES.md>
- Retired agent IDs: <list>
- Primary artifact owner: <paths>
- Session kept: <session id or "respawn from root">
```

### 2. Pick the canonical role

Decide upfront:

| Field | Rule |
| --- | --- |
| **Role name** | User preference, or the more general name |
| **Agent ID** | Keep the session you will retain; or the ID tied to the better assistant config |
| **Artifacts** | Union of deliverables; one primary writer per file |
| **Tools** | Union of allowed tool families; trim duplicates in prose |
| **Handoff targets** | Rewire to surviving role names only |

### 3. Merge ROLES.md sections

One merged section:

```markdown
## <Canonical Role>
- **ID**: <assistant name> (`<canonical-uuid>`)
- **책임**: <combined responsibilities, no duplication>
- **허용 도구**: <union, deduplicated>
- **필수 입력**: <combined>
- **필수 출력**: <combined, note primary files>
- **Handoff**: <updated targets>
```

Delete the retired role sections.

### 4. Sync MISSION.md

Replace the separate role subsections with one subsection matching the merged role.

### 5. Skills

| Situation | Action |
| --- | --- |
| Both had `skills/tf-*` | Merge content into one skill; delete or deprecate the other |
| Only one had a skill | Rename slug if role name changed |
| Neither had skills | Optional: create one if merged role is durable |

### 6. Coordination files

- **KANBAN:** Rewrite `owner:` from retired names → canonical role name
- **HANDOFF:** Add merge notice; future entries use canonical name only
- **RISKS:** Update `Owner:` fields

### 7. Sessions

| Situation | Action |
| --- | --- |
| One active session per merged role | Keep one; stop/delete the other after handoff |
| Both active with in-flight work | Message both; consolidate to one; stop redundant |
| Neither active | Next spawn uses canonical agent ID from root |

Do not leave two org children doing the same merged role without explicit parallel split in ROLES.md.

### 8. Broadcast and verify

Same as layoff: root message, refresh semantics, `getOrg` check, grep for retired role names.
