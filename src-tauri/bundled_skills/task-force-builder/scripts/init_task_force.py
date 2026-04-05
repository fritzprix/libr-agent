#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "role"


def parse_role(raw: str) -> tuple[str, str]:
    if ":" not in raw:
        raise argparse.ArgumentTypeError(
            "Role must use 'Name:Responsibility' format"
        )
    name, responsibility = raw.split(":", 1)
    name = name.strip()
    responsibility = responsibility.strip()
    if not name or not responsibility:
        raise argparse.ArgumentTypeError(
            "Role name and responsibility must both be non-empty"
        )
    return name, responsibility


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content.rstrip() + "\n", encoding="utf-8")


def build_role_skill(
    role_name: str,
    responsibility: str,
    objective: str,
    primary_artifact: str,
    execution_substrate: str,
) -> str:
    role_slug = slugify(role_name)
    return f"""---
name: tf-{role_slug}
description: Specialist role for the current task force. Use when work requires {responsibility.lower()}.
---

# {role_name}

You are the {role_name} for this task force.

## Mission slice

Support the overall objective: {objective}

Own this responsibility: {responsibility}

## Required inputs

- agents.md
- MISSION.md
- ROLES.md
- coordination/KANBAN.md
- coordination/HANDOFF.md

## Required outputs

- {primary_artifact}
- coordination/HANDOFF.md
- coordination/KANBAN.md

## Workflow

1. Read agents.md, MISSION.md, ROLES.md, and the current coordination files.
2. Confirm the task you are acting on in coordination/KANBAN.md.
3. Update or create {primary_artifact}.
4. Record blocked state, risks, or decisions in the proper coordination files.
5. Leave a precise handoff in coordination/HANDOFF.md.

## Refresh awareness

- If the workspace constitution or skills were just changed, do not assume this session already reloaded them.
- Follow the refresh notes in agents.md and .libragent/teamwork.json before continuing.

## Guardrails

- Stay inside your role boundary.
- Do not silently change another role's main artifact.
- If blocked, record the blocker instead of pretending progress happened.
- Execution substrate for this task force: {execution_substrate}
- If explicit org lineage is chosen later, follow `team-org`.
- If scheduled task groups are chosen later, follow `team-sprint`.
"""


def build_agents_instructions(
    objective: str,
    framework: str,
    original_request: str,
    roles: list[tuple[str, str]],
    execution_substrate: str,
) -> str:
    role_names = ", ".join(name for name, _ in roles) if roles else "Coordinator"
    return f"""# Team Workspace Instructions

This workspace is the canonical operating system for the current teamwork run.

## Objective

{objective}

## Original User Request

{original_request}

## Collaboration Model

{framework}

## Execution Substrate

{execution_substrate}

## Execution Specialist Skills

- Plain child sessions: use `subagent-session-delegation` when delegation mechanics matter.
- Explicit org lineage: use `team-org`.
- Scheduled task groups: use `team-sprint`.

## Active Roles

{role_names}

## Canonical Files

- `MISSION.md` - objective, hard constraints, definition of done, and required deliverables
- `ROLES.md` - role boundaries, allowed writes, and ownership
- `coordination/KANBAN.md` - task state and ownership
- `coordination/HANDOFF.md` - append-only handoffs between roles
- `coordination/DECISIONS.md` - durable decisions that affect downstream work
- `coordination/RISKS.md` - concrete active risks and mitigation
- `coordination/DISCUSSION.md` - working notes that are not final decisions
- `.libragent/teamwork.json` - machine-readable teamwork manifest for tooling and UI

## Required Operating Rules

1. Read `MISSION.md`, `ROLES.md`, and `coordination/KANBAN.md` before meaningful work.
2. Claim or update work in `coordination/KANBAN.md` before starting execution.
3. Write durable status changes to the canonical coordination files, not only to chat.
4. Append handoffs to `coordination/HANDOFF.md` instead of rewriting previous entries.
5. Promote durable choices into `coordination/DECISIONS.md`; do not leave final decisions buried in `coordination/DISCUSSION.md`.
6. Record blockers and active risks honestly in `coordination/KANBAN.md` and `coordination/RISKS.md`.
7. Stay inside your role boundary. Do not silently rewrite another role's primary artifact.
8. The governing coordinator must keep working in this workspace.
9. If the scaffold is incomplete or stale, repair the workspace constitution before pushing new directives.
10. If this teamwork run later uses explicit org lineage, org-visible child sessions should normally share this workspace.

## Refresh And Resume Notes

- Changes to `agents.md` and other workspace constitution files do not instantly rewrite the current session prompt.
- Newly created workspace skills apply in a later execution step, not retroactively in the same turn.
- Use `.libragent/teamwork.json` as the machine-readable contract for execution mode and refresh expectations.
"""


def build_teamwork_manifest(
    team_name: str,
    objective: str,
    original_request: str,
    framework: str,
    roles: list[tuple[str, str]],
) -> str:
    manifest = {
        "schemaVersion": 2,
        "teamName": team_name,
        "objective": objective,
        "originalUserRequest": original_request,
        "framework": framework,
        "executionSubstrate": {
            "mode": "plain-child-sessions",
            "workspacePolicy": {
                "plainChildSessions": "isolated-by-default",
                "explicitOrgLineage": "share-coordinator-workspace-by-default",
                "scheduledTaskGroups": "workspace-defined-per-group",
            },
            "specialistSkills": {
                "plainChildSessions": "subagent-session-delegation",
                "explicitOrgLineage": "team-org",
                "scheduledTaskGroups": "team-sprint",
            },
            "orgLineage": {
                "intended": False,
                "rootAction": "createOrg",
                "childAction": "startSession",
                "childArgs": {"includeCurrentOrg": True},
                "compatibilityAlias": "spawnOrgAgent",
                "workspaceSharing": "inherit-root-workspace-by-default",
            },
            "scheduledTaskGroups": {
                "intended": False,
                "notes": "Use scheduled task groups for recurring or cron-like collaboration, not org lineage.",
            },
        },
        "refreshSemantics": {
            "workspaceInstructions": "Changes to agents.md and related workspace constitution files apply in a later execution step, not in the current turn.",
            "workspaceSkills": "New workspace skills apply in a later execution step, not retroactively in the same turn.",
        },
        "constitutionAdoption": {
            "coordinatorMustShareScaffoldRoot": True,
            "rule": "Continue coordination in the same workspace where the constitution was created.",
        },
        "roles": [
            {"name": role_name, "responsibility": responsibility}
            for role_name, responsibility in roles
        ],
    }
    return json.dumps(manifest, indent=2, ensure_ascii=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Scaffold a task force workspace")
    parser.add_argument(
        "--output",
        required=True,
        help=(
            "Workspace to scaffold into. Use the current workspace for the current teamwork run."
        ),
    )
    parser.add_argument("--objective", required=True, help="Overall task force objective")
    parser.add_argument(
        "--request",
        help="Original user request to preserve in the teamwork scaffold. Defaults to the objective when omitted.",
    )
    parser.add_argument(
        "--team-name",
        help="Display name for the task force. Defaults to the workspace directory name.",
    )
    parser.add_argument(
        "--framework",
        required=True,
        choices=["sequential", "hub-and-spoke", "swarm"],
        help="Primary collaboration model",
    )
    parser.add_argument(
        "--role",
        action="append",
        type=parse_role,
        default=[],
        help="Role in 'Name:Responsibility' format. Repeat for multiple roles.",
    )
    args = parser.parse_args()

    workspace = Path(args.output).expanduser().resolve()
    workspace.mkdir(parents=True, exist_ok=True)
    original_request = args.request.strip() if args.request else args.objective
    team_name = args.team_name.strip() if args.team_name else workspace.name
    role_definitions = args.role or [
        ("Coordinator", "Refine the team structure and assign work")
    ]
    execution_substrate = (
        "Plain child sessions by default. "
        "Use createOrg(...) plus startSession(..., includeCurrentOrg=true) when explicit org lineage is required, then follow team-org; org-visible children should normally share the coordinator's workspace. "
        "Use createScheduledTask(...) and related scheduled_task tools for recurring automation, then follow team-sprint."
    )

    write(
        workspace / "MISSION.md",
        f"""# Mission

## Team
{team_name}

## Objective
{args.objective}

## Original User Request
{original_request}

## Collaboration Model
{args.framework}

## Execution Notes
- Default execution substrate: plain child sessions via `startSession(...)`
- Explicit org lineage: `createOrg(...)` once from the root, then `startSession(..., includeCurrentOrg=true)` for org-visible children, sharing the coordinator's workspace by default, then follow `team-org`
- Recurring automation: `createScheduledTask(...)` and related scheduled-task tools, kept separate from org lineage, then follow `team-sprint`

## Definition of Done
- Replace this list with concrete success criteria.

## Deliverables
- Replace this list with required artifacts.
""",
    )

    role_sections = []
    for role_name, responsibility in role_definitions:
        role_sections.append(
            f"""## {role_name}
- Mission slice: {responsibility}
- Reads: MISSION.md, coordination/KANBAN.md, coordination/HANDOFF.md
- Writes: coordination/KANBAN.md, coordination/HANDOFF.md, docs/{slugify(role_name).upper()}_NOTES.md
"""
        )

        primary_artifact = f"docs/{slugify(role_name).upper()}_NOTES.md"
        write(
            workspace / "skills" / f"tf-{slugify(role_name)}" / "SKILL.md",
            build_role_skill(
                role_name,
                responsibility,
                args.objective,
                primary_artifact,
                execution_substrate,
            ),
        )
        write(
            workspace / primary_artifact,
            f"# {role_name} Notes\n\n## Responsibility\n{responsibility}\n",
        )

    write(workspace / "ROLES.md", "# Roles\n\n" + "\n".join(role_sections))
    write(
        workspace / "agents.md",
        build_agents_instructions(
            args.objective,
            args.framework,
            original_request,
            role_definitions,
            execution_substrate,
        ),
    )
    write(
        workspace / ".libragent" / "teamwork.json",
        build_teamwork_manifest(
            team_name,
            args.objective,
            original_request,
            args.framework,
            role_definitions,
        ),
    )

    write(
        workspace / "coordination" / "KANBAN.md",
        """# KANBAN

## Backlog
- [ ] Replace with real tasks - owner: unassigned

## In Progress

## Blocked

## Done
""",
    )
    write(
        workspace / "coordination" / "HANDOFF.md",
        "# HANDOFF\n\n## Initial setup\n- Created task force scaffold.\n",
    )
    write(
        workspace / "coordination" / "DECISIONS.md",
        "# DECISIONS\n\n## Initial decision\n- Collaboration model selected during scaffold creation.\n",
    )
    write(
        workspace / "coordination" / "RISKS.md",
        "# RISKS\n\n- Replace with real risks once planning starts.\n",
    )
    write(
        workspace / "coordination" / "DISCUSSION.md",
        "# DISCUSSION\n\nUse this file for working notes that are not final decisions.\n",
    )

    print(f"Created task force workspace at: {workspace}")


if __name__ == "__main__":
    main()
