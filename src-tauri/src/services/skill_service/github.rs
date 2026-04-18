use super::contracts::GitHubRepoSpec;
use std::path::{Component, PathBuf};
use url::Url;

fn parse_github_query_subpath(raw_subpath: &str) -> Result<Option<PathBuf>, String> {
    if raw_subpath.is_empty() {
        return Ok(None);
    }

    let subpath = PathBuf::from(raw_subpath);
    if subpath.is_absolute() {
        return Err("GitHub path query must be a relative subdirectory".to_string());
    }
    if subpath.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err("GitHub path query cannot escape the downloaded repository".to_string());
    }

    Ok(Some(subpath))
}

pub fn parse_github_repo_url(repo_url: &str) -> Result<GitHubRepoSpec, String> {
    let parsed = Url::parse(repo_url).map_err(|error| format!("Invalid GitHub URL: {}", error))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "GitHub URL must include a host".to_string())?;
    if host != "github.com" && host != "www.github.com" {
        return Err("Only github.com repository URLs are supported".to_string());
    }

    let segments = parsed
        .path_segments()
        .ok_or_else(|| "GitHub URL is missing path segments".to_string())?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.len() < 2 {
        return Err("GitHub URL must point to a repository".to_string());
    }

    let owner = segments[0].to_string();
    let repo = segments[1].trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("GitHub repository URL is incomplete".to_string());
    }

    let mut branch_query = None;
    let mut path_query = None;
    for (key, value) in parsed.query_pairs() {
        if key == "ref" && !value.is_empty() {
            branch_query = Some(value.into_owned());
        } else if key == "path" && !value.is_empty() {
            path_query = parse_github_query_subpath(value.as_ref())?;
        }
    }

    let (branch, subpath) = if branch_query.is_some() || path_query.is_some() {
        (branch_query, path_query)
    } else if segments.len() > 2 && segments[2] == "tree" {
        if segments.len() < 4 {
            return Err("GitHub tree URL must include a branch name".to_string());
        }
        if segments.len() > 4 {
            return Err(
                "Ambiguous GitHub tree URL. Use ?ref=<branch> and optional ?path=<subdirectory> for branches containing '/' or subdirectory installs.".to_string(),
            );
        }
        (Some(segments[3].to_string()), None)
    } else {
        (None, None)
    };

    Ok(GitHubRepoSpec {
        owner,
        repo,
        branch,
        subpath,
    })
}
