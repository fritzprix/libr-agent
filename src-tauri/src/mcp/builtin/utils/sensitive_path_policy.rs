use super::SecurityValidator;
use glob::Pattern;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use std::path::{Component, Path};

static SENSITIVE_PATH_POLICY: Lazy<SensitivePathPolicy> = Lazy::new(load_sensitive_path_policy);

pub fn matches_sensitive_path_policy(candidate_path: &Path) -> bool {
    SENSITIVE_PATH_POLICY.matches(candidate_path)
}

#[derive(Debug, Deserialize)]
struct SensitivePathPolicyConfig {
    rules: Vec<SensitivePathRuleConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleScope {
    Absolute,
    Home,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleMatch {
    Exact,
    Glob,
    Pattern,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleOs {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Deserialize)]
struct SensitivePathRuleConfig {
    scope: SensitivePathRuleScope,
    #[serde(rename = "match")]
    matcher: SensitivePathRuleMatch,
    value: String,
    #[serde(default)]
    os: Vec<SensitivePathRuleOs>,
}

#[derive(Debug)]
struct SensitivePathPolicy {
    rules: Vec<CompiledSensitivePathRule>,
}

#[derive(Debug)]
struct CompiledSensitivePathRule {
    scope: SensitivePathRuleScope,
    matcher: CompiledSensitivePathRuleMatcher,
    os: Vec<SensitivePathRuleOs>,
}

#[derive(Debug)]
enum CompiledSensitivePathRuleMatcher {
    Exact(String),
    Glob(Pattern),
    Pattern(Regex),
}

impl SensitivePathPolicy {
    fn matches(&self, candidate_path: &Path) -> bool {
        self.rules.iter().any(|rule| rule.matches(candidate_path))
    }
}

impl CompiledSensitivePathRule {
    fn matches(&self, candidate_path: &Path) -> bool {
        if !self.os.is_empty() && !self.os.contains(&current_policy_os()) {
            return false;
        }

        let target = match self.scope {
            SensitivePathRuleScope::Absolute => normalize_policy_path(candidate_path),
            SensitivePathRuleScope::Home => {
                let Some(relative_path) = relative_path_under_any_user_home(candidate_path) else {
                    return false;
                };
                normalize_policy_value(&relative_path)
            }
        };

        match &self.matcher {
            CompiledSensitivePathRuleMatcher::Exact(value) => target == *value,
            CompiledSensitivePathRuleMatcher::Glob(pattern) => pattern.matches(&target),
            CompiledSensitivePathRuleMatcher::Pattern(regex) => regex.is_match(&target),
        }
    }
}

fn load_sensitive_path_policy() -> SensitivePathPolicy {
    let config: SensitivePathPolicyConfig =
        serde_json::from_str(include_str!("../sensitive_path_policy.json"))
            .expect("sensitive path policy JSON must be valid");

    let rules = config
        .rules
        .into_iter()
        .map(|rule| {
            let matcher =
                compile_sensitive_path_matcher(rule.matcher, &rule.value).unwrap_or_else(|error| {
                    panic!(
                        "Invalid sensitive path policy rule ({:?} {:?} {:?}): {}",
                        rule.scope, rule.matcher, rule.value, error
                    )
                });

            CompiledSensitivePathRule {
                scope: rule.scope,
                matcher,
                os: rule.os,
            }
        })
        .collect();

    SensitivePathPolicy { rules }
}

fn compile_sensitive_path_matcher(
    matcher: SensitivePathRuleMatch,
    value: &str,
) -> Result<CompiledSensitivePathRuleMatcher, String> {
    let normalized_value = normalize_policy_value(value);

    match matcher {
        SensitivePathRuleMatch::Exact => {
            Ok(CompiledSensitivePathRuleMatcher::Exact(normalized_value))
        }
        SensitivePathRuleMatch::Glob => Pattern::new(&normalized_value)
            .map(CompiledSensitivePathRuleMatcher::Glob)
            .map_err(|error| error.msg.to_string()),
        SensitivePathRuleMatch::Pattern => Regex::new(value)
            .map(CompiledSensitivePathRuleMatcher::Pattern)
            .map_err(|error| error.to_string()),
    }
}

fn normalize_policy_path(path: &Path) -> String {
    normalize_policy_value(&path.to_string_lossy())
}

fn normalize_policy_value(value: &str) -> String {
    let normalized =
        normalize_macos_private_aliases(&SecurityValidator::normalize_path_separators(value));
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn normalize_macos_private_aliases(value: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        for public_root in ["/etc", "/var", "/tmp"] {
            let private_root = format!("/private{public_root}");
            if value == private_root {
                return public_root.to_string();
            }
            if let Some(suffix) = value.strip_prefix(&(private_root.clone() + "/")) {
                return format!("{public_root}/{suffix}");
            }
        }
    }

    value.to_string()
}

fn current_policy_os() -> SensitivePathRuleOs {
    #[cfg(target_os = "linux")]
    {
        SensitivePathRuleOs::Linux
    }
    #[cfg(target_os = "macos")]
    {
        SensitivePathRuleOs::Macos
    }
    #[cfg(target_os = "windows")]
    {
        SensitivePathRuleOs::Windows
    }
}

fn component_eq(component: &Component<'_>, expected: &str) -> bool {
    matches!(component, Component::Normal(name) if normalize_policy_value(&name.to_string_lossy()) == normalize_policy_value(expected))
}

fn relative_path_under_any_user_home(path: &Path) -> Option<String> {
    if let Some(current_home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&current_home) {
            return Some(join_normal_components(stripped.components()));
        }
    }

    let components: Vec<_> = path.components().collect();

    #[cfg(unix)]
    {
        match components.as_slice() {
            [Component::RootDir, user_home, rest @ ..] if component_eq(user_home, "root") => {
                return Some(join_normal_component_slice(rest));
            }
            [Component::RootDir, home_root, Component::Normal(_), rest @ ..]
                if component_eq(home_root, "home") || component_eq(home_root, "Users") =>
            {
                return Some(join_normal_component_slice(rest));
            }
            _ => {}
        }
    }

    #[cfg(windows)]
    {
        match components.as_slice() {
            [Component::Prefix(_), Component::RootDir, users_root, Component::Normal(_), rest @ ..]
                if component_eq(users_root, "Users")
                    || component_eq(users_root, "Documents and Settings") =>
            {
                return Some(join_normal_component_slice(rest));
            }
            _ => {}
        }
    }

    None
}

fn join_normal_components<'a>(components: impl Iterator<Item = Component<'a>>) -> String {
    components
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn join_normal_component_slice(components: &[Component<'_>]) -> String {
    components
        .iter()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
