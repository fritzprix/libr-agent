//! Host-side staging for `POST /api/sessions/<id>/skills/install/<token>`.
//!
//! The request carries a skill as a flat list of files. Nothing from the
//! request is trusted: paths are validated component by component, sizes are
//! capped, and the files are only written into a caller-provided scratch
//! directory. Installing into the user skills folder is left to
//! `skill_service::import_user_skills`, which validates the SKILL.md
//! frontmatter again.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 6 * 1024 * 1024;
const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Deserialize)]
pub struct SkillInstallRequest {
    pub files: Vec<SkillInstallFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstallFile {
    pub path: String,
    pub content_base64: String,
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let invalid = || format!("invalid file path: {path:?}");
    if path.is_empty() || path.contains('\\') || path.contains('\0') {
        return Err(invalid());
    }
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(':') {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Validates `request` and writes its files under `dest`.
///
/// Requires a root `SKILL.md` and rejects any other `SKILL.md`, so one
/// request always installs exactly one skill.
pub fn stage_files(request: &SkillInstallRequest, dest: &Path) -> Result<(), String> {
    if request.files.is_empty() {
        return Err("no files to install".to_string());
    }
    if request.files.len() > MAX_FILES {
        return Err(format!("too many files (max {MAX_FILES})"));
    }

    let mut seen = HashSet::new();
    let mut decoded = Vec::with_capacity(request.files.len());
    let mut total = 0usize;
    for file in &request.files {
        validate_relative_path(&file.path)?;
        if !seen.insert(file.path.to_ascii_lowercase()) {
            return Err(format!("duplicate file path: {:?}", file.path));
        }
        let is_skill_file = file
            .path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(SKILL_FILE));
        if is_skill_file && file.path != SKILL_FILE {
            return Err(format!(
                "{SKILL_FILE} is only allowed at the skill root: {:?}",
                file.path
            ));
        }
        let bytes = STANDARD
            .decode(&file.content_base64)
            .map_err(|error| format!("invalid base64 for {:?}: {error}", file.path))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(format!("file too large: {:?}", file.path));
        }
        total += bytes.len();
        if total > MAX_TOTAL_BYTES {
            return Err("skill too large".to_string());
        }
        decoded.push((&file.path, bytes));
    }
    if !seen.contains(&SKILL_FILE.to_ascii_lowercase()) {
        return Err(format!("{SKILL_FILE} is required at the skill root"));
    }

    for (path, bytes) in decoded {
        let target = dest.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&target, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, content: &str) -> SkillInstallFile {
        SkillInstallFile {
            path: path.to_string(),
            content_base64: STANDARD.encode(content),
        }
    }

    fn stage(files: Vec<SkillInstallFile>) -> (tempfile::TempDir, Result<(), String>) {
        let dir = tempfile::tempdir().unwrap();
        let result = stage_files(&SkillInstallRequest { files }, dir.path());
        (dir, result)
    }

    #[test]
    fn stages_nested_files() {
        let (dir, result) = stage(vec![
            file("SKILL.md", "---\nname: demo\n---\n"),
            file("scripts/run.py", "print(1)"),
        ]);
        result.unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/run.py")).unwrap(),
            "print(1)"
        );
    }

    #[test]
    fn rejects_unsafe_paths() {
        for bad in [
            "../evil", "/abs", "a/../b", "a//b", "./a", "a\\b", "C:/x", "", "a/",
        ] {
            let (dir, result) = stage(vec![file("SKILL.md", "x"), file(bad, "x")]);
            assert!(result.is_err(), "{bad:?} must be rejected");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "{bad:?}");
        }
    }

    #[test]
    fn requires_single_root_skill_file() {
        assert!(stage(vec![file("README.md", "x")]).1.is_err());
        assert!(
            stage(vec![file("SKILL.md", "x"), file("nested/SKILL.md", "x")])
                .1
                .is_err()
        );
        assert!(
            stage(vec![file("SKILL.md", "x"), file("nested/skill.MD", "x")])
                .1
                .is_err()
        );
        assert!(stage(vec![file("SKILL.md", "x"), file("skill.md", "x")])
            .1
            .is_err());
    }

    #[test]
    fn rejects_bad_base64_and_oversize() {
        let bad = SkillInstallFile {
            path: "SKILL.md".into(),
            content_base64: "!!!".into(),
        };
        assert!(stage(vec![bad]).1.is_err());

        let big = "x".repeat(MAX_FILE_BYTES + 1);
        assert!(stage(vec![file("SKILL.md", &big)]).1.is_err());
    }
}
