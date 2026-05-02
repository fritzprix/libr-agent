#![cfg(not(windows))]

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("bundled_skills")
        .join("teamwork")
        .join("scripts")
        .join("init_task_force.py")
}

fn base_command(output_dir: &std::path::Path) -> Command {
    let mut command = Command::new("python3");
    command
        .arg(script_path())
        .arg("--output")
        .arg(output_dir)
        .arg("--team-name")
        .arg("Research Strike Team")
        .arg("--objective")
        .arg("Build a reusable research and implementation team")
        .arg("--request")
        .arg("Research the space, structure findings, and hand implementation-ready guidance to coding specialists.")
        .arg("--framework")
        .arg("hub-and-spoke")
        .arg("--role")
        .arg("Coordinator:Own planning, prioritization, and integration");
    command
}

#[test]
fn scaffold_script_refuses_git_worktree_without_opt_in() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    std::fs::create_dir_all(temp_dir.path().join(".git")).expect(".git dir should be created");

    let output = base_command(temp_dir.path())
        .output()
        .expect("script should run");

    assert!(
        !output.status.success(),
        "script should reject git worktrees by default"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to scaffold inside a Git worktree"),
        "expected git-worktree refusal message, got: {stderr}"
    );
}

#[test]
fn scaffold_script_refuses_nested_git_worktree_path_without_opt_in() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    std::fs::create_dir_all(temp_dir.path().join(".git")).expect(".git dir should be created");
    let nested = temp_dir.path().join("teamwork");

    let output = base_command(&nested).output().expect("script should run");

    assert!(
        !output.status.success(),
        "script should reject nested paths inside a git worktree by default"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to scaffold inside a Git worktree"),
        "expected git-worktree refusal message, got: {stderr}"
    );
    assert!(
        !nested.exists(),
        "script should not create nested output paths before refusing"
    );
}

#[test]
fn scaffold_script_allows_git_worktree_with_explicit_opt_in() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    std::fs::create_dir_all(temp_dir.path().join(".git")).expect(".git dir should be created");

    let output = base_command(temp_dir.path())
        .arg("--allow-git-worktree")
        .output()
        .expect("script should run");

    assert!(
        output.status.success(),
        "script should allow git repo root when explicitly opted in: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        temp_dir.path().join("MISSION.md").exists(),
        "expected scaffold file to be created when opt-in is set"
    );
}

#[test]
fn scaffold_script_writes_expected_teamwork_manifest_contract() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let output_dir = temp_dir.path().join("teamwork-workspace");
    std::fs::create_dir_all(&output_dir).expect("output dir should be created");

    let output = base_command(&output_dir)
        .arg("--execution-substrate")
        .arg("org")
        .output()
        .expect("script should run");

    assert!(
        output.status.success(),
        "script should scaffold successfully outside a git worktree: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_path = output_dir.join(".libragent").join("teamwork.json");
    let manifest_raw =
        std::fs::read_to_string(&manifest_path).expect("teamwork manifest should exist");
    let manifest: Value =
        serde_json::from_str(&manifest_raw).expect("teamwork manifest should be valid JSON");

    assert_eq!(manifest["schemaVersion"], 2);
    assert_eq!(manifest["executionSubstrate"]["mode"], "org");
    assert_eq!(manifest["executionSubstrate"]["specialistSkill"], "org");
    assert_eq!(
        manifest["executionSubstrate"]["workspacePolicy"]["explicitOrgLineage"],
        "share-governing-teamwork-workspace-by-default"
    );
    assert_eq!(
        manifest["executionSubstrate"]["orgLineage"]["childArgs"]["includeCurrentOrg"],
        true
    );
    assert_eq!(
        manifest["constitutionAdoption"]["coordinatorMustShareScaffoldRoot"],
        true
    );
    assert_eq!(
        manifest["constitutionAdoption"]["rule"],
        "Continue coordination in the dedicated teamwork workspace where the constitution was created."
    );
}
