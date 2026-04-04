#!/usr/bin/env python3

from __future__ import annotations

import argparse
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

- MISSION.md
- ROLES.md
- coordination/KANBAN.md
- coordination/HANDOFF.md

## Required outputs

- {primary_artifact}
- coordination/HANDOFF.md
- coordination/KANBAN.md

## Workflow

1. Read MISSION.md, ROLES.md, and the current coordination files.
2. Confirm the task you are acting on in coordination/KANBAN.md.
3. Update or create {primary_artifact}.
4. Record blocked state, risks, or decisions in the proper coordination files.
5. Leave a precise handoff in coordination/HANDOFF.md.

## Guardrails

- Stay inside your role boundary.
- Do not silently change another role's main artifact.
- If blocked, record the blocker instead of pretending progress happened.
"""


def main() -> None:
    parser = argparse.ArgumentParser(description="Scaffold a task force workspace")
    parser.add_argument("--output", required=True, help="Workspace directory to create")
    parser.add_argument("--objective", required=True, help="Overall task force objective")
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

    write(
        workspace / "MISSION.md",
        f"""# Mission

## Objective
{args.objective}

## Collaboration Model
{args.framework}

## Definition of Done
- Replace this list with concrete success criteria.

## Deliverables
- Replace this list with required artifacts.
""",
    )

    role_sections = []
    if not args.role:
        role_sections.append(
            "## Coordinator\n- Mission slice: refine the team structure and assign work\n"
            "- Reads: MISSION.md, coordination/KANBAN.md\n"
            "- Writes: ROLES.md, coordination/KANBAN.md, coordination/HANDOFF.md"
        )

    for role_name, responsibility in args.role:
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
            build_role_skill(role_name, responsibility, args.objective, primary_artifact),
        )
        write(
            workspace / primary_artifact,
            f"# {role_name} Notes\n\n## Responsibility\n{responsibility}\n",
        )

    write(workspace / "ROLES.md", "# Roles\n\n" + "\n".join(role_sections))

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
