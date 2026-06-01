# Bundled Skill Implementation Notes

Use this file when the new bundled skill needs deployment-aware wording.

## File map

- `src-tauri/build.rs` + `src-tauri/build_support/bundled_skills.rs`
  - Build-time mirror of valid bundled skill directories into packaged resources
- `src-tauri/src/lifecycle/app_setup.rs`
  - Runtime sync from packaged bundled skills into managed `system_skills`
- `src-tauri/src/services/skill_service/directories.rs`
  - System, user, assistant, and workspace skill directory resolution
- `src-tauri/src/agent/references/skill.rs`
  - `@skill:name` expansion and Base Directory injection

## Deployment model

Bundled skills live in the repo under `src-tauri/bundled_skills/<skill-name>/`.

At build time, valid skill directories are mirrored into packaged resources. At app startup, the packaged bundled snapshot is synced into the managed system skill directory under app data.

This means:

- Repo paths are authoring paths only
- The deployed skill path is different from the workspace root
- User-facing or agent-facing instructions must describe the deployed behavior, not the repo layout

## Path model

There are multiple path domains:

1. **Skill Base Directory** - the deployed directory containing `SKILL.md`
2. **Workspace Root** - the agent's working tree and default `./` for workspace tools
3. **Persistent shell cwd** - can drift after `Set-Location`, unlike workspace file tools

`@skill:name` expansion injects the absolute Base Directory and tells the agent that relative paths in the skill must be resolved against it.

Do not assume:

- `scripts/...` means workspace-relative
- `./` in shell examples points at the deployed skill directory
- repo-only paths remain valid after packaging

If an example touches both path domains, make that explicit:

```text
python <skill-base-dir>/scripts/run.py --root .
```

Here the script lives in the bundled skill, while `--root .` points at the workspace.

## Update model

Managed `system_skills` sync uses a per-skill hash manifest built from file paths and file contents.

- Adding or changing `.force_update` changes the skill hash
- That causes the managed copy to be replaced on the next app start
- `.force_update` is useful when an existing installed copy must be refreshed

It is not a dedicated runtime switch. It works because the hash changes.
