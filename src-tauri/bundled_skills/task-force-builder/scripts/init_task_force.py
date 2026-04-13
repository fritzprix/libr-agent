#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

SUBSTRATE_PLAIN = "plain-child-sessions"
SUBSTRATE_ORG = "org"
SUBSTRATE_SCHEDULED = "scheduled"

SUBSTRATE_CHOICES = [SUBSTRATE_PLAIN, SUBSTRATE_ORG, SUBSTRATE_SCHEDULED]

SUBSTRATE_DISPLAY = {
    SUBSTRATE_PLAIN: (
        "Plain child sessions via startSession(...). "
        "Use subagent-session-delegation for delegation mechanics when needed."
    ),
    SUBSTRATE_ORG: (
        "Explicit org lineage via createOrg(...) once from the root session, "
        "then startSession(...) for org-visible children. "
        "Org-visible children share the coordinator's workspace by default. "
        "Follow team-org for org-specific operating rules."
    ),
    SUBSTRATE_SCHEDULED: (
        "Scheduled task groups via createScheduledTask(...) and related scheduled_task tools. "
        "Use a stable groupName for the first task and groupId for subsequent tasks in the same group. "
        "Follow team-sprint for scheduled-group operating rules."
    ),
}

SUBSTRATE_SPECIALIST_SKILL = {
    SUBSTRATE_PLAIN: "subagent-session-delegation",
    SUBSTRATE_ORG: "team-org",
    SUBSTRATE_SCHEDULED: "team-sprint",
}


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
    substrate_mode: str = SUBSTRATE_PLAIN,
) -> str:
    role_names = ", ".join(name for name, _ in roles) if roles else "Coordinator"
    substrate_text = SUBSTRATE_DISPLAY[substrate_mode]
    specialist_skill = SUBSTRATE_SPECIALIST_SKILL[substrate_mode]
    return f"""# Team Workspace Instructions

This workspace is the canonical operating system for the current teamwork run.

## Objective

{objective}

## Original User Request

{original_request}

## Collaboration Model

{framework}

## Execution Substrate

{substrate_text}

Active specialist skill: `{specialist_skill}`

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

1. Read `.libragent/teamwork.json` to confirm the active execution substrate before working.
2. Read `MISSION.md`, `ROLES.md`, and `coordination/KANBAN.md` before meaningful work.
3. Claim or update work in `coordination/KANBAN.md` before starting execution.
4. Write durable status changes to the canonical coordination files, not only to chat.
5. Append handoffs to `coordination/HANDOFF.md` instead of rewriting previous entries.
6. Promote durable choices into `coordination/DECISIONS.md`; do not leave final decisions buried in `coordination/DISCUSSION.md`.
7. Record blockers and active risks honestly in `coordination/KANBAN.md` and `coordination/RISKS.md`.
8. Stay inside your role boundary. Do not silently rewrite another role's primary artifact.
9. The governing coordinator must keep working in this workspace.
10. If the scaffold is incomplete or stale, repair the workspace constitution before pushing new directives.
11. If this teamwork run uses explicit org lineage, org-visible child sessions should normally share this workspace.

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
    execution_substrate: str = SUBSTRATE_PLAIN,
) -> str:
    is_org = execution_substrate == SUBSTRATE_ORG
    is_scheduled = execution_substrate == SUBSTRATE_SCHEDULED
    manifest = {
        "schemaVersion": 2,
        "teamName": team_name,
        "objective": objective,
        "originalUserRequest": original_request,
        "framework": framework,
        "executionSubstrate": {
            "mode": execution_substrate,
            "specialistSkill": SUBSTRATE_SPECIALIST_SKILL[execution_substrate],
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
                "intended": is_org,
                "rootAction": "createOrg",
                "childAction": "startSession",
                "childArgs": {},
                "workspaceSharing": "inherit-root-workspace-by-default",
            },
            "scheduledTaskGroups": {
                "intended": is_scheduled,
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
        "--execution-substrate",
        choices=SUBSTRATE_CHOICES,
        default=SUBSTRATE_PLAIN,
        help=(
            "Execution substrate for this task force. "
            f"Choices: {', '.join(SUBSTRATE_CHOICES)}. "
            "Defaults to plain-child-sessions. "
            "Use 'org' for explicit org lineage (team-org). "
            "Use 'scheduled' for recurring automation (team-sprint)."
        ),
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
    substrate_mode = args.execution_substrate
    substrate_text = SUBSTRATE_DISPLAY[substrate_mode]

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

## Execution Substrate
{substrate_text}

Active specialist skill: `{SUBSTRATE_SPECIALIST_SKILL[substrate_mode]}`

## Execution Notes
- Plain child sessions: `startSession(...)`, use `subagent-session-delegation` for delegation mechanics
- Explicit org lineage: `createOrg(...)` once from root, then `startSession(...)`, follow `team-org`
- Recurring automation: `createScheduledTask(...)` and related scheduled-task tools, follow `team-sprint`

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
                substrate_text,
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
            substrate_mode,
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
            substrate_mode,
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
