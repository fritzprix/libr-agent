use serde::{Deserialize, Serialize};

/// Installation guide for a development tool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationGuide {
    pub tool: String,
    pub platform: String,
    pub steps: Vec<InstallationStep>,
    pub verification: String,
    pub notes: Vec<String>,
}

/// A single installation step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationStep {
    pub description: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// Get installation guide for a specific tool and platform
///
/// # Arguments
/// * `tool` - Tool name (e.g., "node", "python", "uv", "docker", "git")
/// * `platform` - Target platform (e.g., "windows", "darwin", "linux", or "auto")
///
/// # Returns
/// Installation guide with steps, verification command, and notes
pub fn get_installation_guide(tool: &str, platform: Option<&str>) -> InstallationGuide {
    let target_platform = platform.unwrap_or("auto");
    let actual_platform = if target_platform == "auto" {
        super::platform::detect_current_platform().os
    } else {
        target_platform.to_string()
    };

    match tool {
        "node" => get_node_guide(&actual_platform),
        "python" => get_python_guide(&actual_platform),
        "uv" => get_uv_guide(&actual_platform),
        "docker" => get_docker_guide(&actual_platform),
        "git" => get_git_guide(&actual_platform),
        _ => InstallationGuide {
            tool: tool.to_string(),
            platform: actual_platform,
            steps: vec![],
            verification: String::new(),
            notes: vec![format!(
                "No installation guide available for tool: {}",
                tool
            )],
        },
    }
}

fn get_node_guide(platform: &str) -> InstallationGuide {
    let steps = match platform {
        "windows" => vec![
            InstallationStep {
                description: "Download Node.js installer from official website".to_string(),
                command: None,
                url: Some("https://nodejs.org/en/download/".to_string()),
            },
            InstallationStep {
                description: "Run the installer and follow the setup wizard".to_string(),
                command: None,
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("node --version".to_string()),
                url: None,
            },
        ],
        "darwin" => vec![
            InstallationStep {
                description: "Install using Homebrew (recommended)".to_string(),
                command: Some("brew install node".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Or download from official website".to_string(),
                command: None,
                url: Some("https://nodejs.org/en/download/".to_string()),
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("node --version".to_string()),
                url: None,
            },
        ],
        _ => vec![
            InstallationStep {
                description: "Install using package manager".to_string(),
                command: Some("sudo apt-get install nodejs npm".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Or use Node Version Manager (nvm)".to_string(),
                command: Some("curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("node --version".to_string()),
                url: None,
            },
        ],
    };

    InstallationGuide {
        tool: "node".to_string(),
        platform: platform.to_string(),
        steps,
        verification: "node --version && npm --version".to_string(),
        notes: vec![
            "Node.js includes npm (Node Package Manager)".to_string(),
            "Recommended version: LTS (Long Term Support)".to_string(),
        ],
    }
}

fn get_python_guide(platform: &str) -> InstallationGuide {
    let steps = match platform {
        "windows" => vec![
            InstallationStep {
                description: "Download Python installer from official website".to_string(),
                command: None,
                url: Some("https://www.python.org/downloads/".to_string()),
            },
            InstallationStep {
                description: "Run installer and check 'Add Python to PATH'".to_string(),
                command: None,
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("python --version".to_string()),
                url: None,
            },
        ],
        "darwin" => vec![
            InstallationStep {
                description: "Install using Homebrew".to_string(),
                command: Some("brew install python3".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("python3 --version".to_string()),
                url: None,
            },
        ],
        _ => vec![
            InstallationStep {
                description: "Install using package manager".to_string(),
                command: Some("sudo apt-get install python3 python3-pip".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("python3 --version".to_string()),
                url: None,
            },
        ],
    };

    InstallationGuide {
        tool: "python".to_string(),
        platform: platform.to_string(),
        steps,
        verification: "python --version && pip --version".to_string(),
        notes: vec![
            "Python 3.8+ recommended".to_string(),
            "pip is included with Python 3.4+".to_string(),
        ],
    }
}

fn get_uv_guide(platform: &str) -> InstallationGuide {
    let steps = match platform {
        "windows" => vec![
            InstallationStep {
                description: "Install using PowerShell".to_string(),
                command: Some(
                    "powershell -c \"irm https://astral.sh/uv/install.ps1 | iex\"".to_string(),
                ),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("uv --version".to_string()),
                url: None,
            },
        ],
        _ => vec![
            InstallationStep {
                description: "Install using curl".to_string(),
                command: Some("curl -LsSf https://astral.sh/uv/install.sh | sh".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("uv --version".to_string()),
                url: None,
            },
        ],
    };

    InstallationGuide {
        tool: "uv".to_string(),
        platform: platform.to_string(),
        steps,
        verification: "uv --version".to_string(),
        notes: vec![
            "uv is an extremely fast Python package installer and resolver".to_string(),
            "Alternative to pip and pip-tools".to_string(),
        ],
    }
}

fn get_docker_guide(platform: &str) -> InstallationGuide {
    let steps = match platform {
        "windows" => vec![
            InstallationStep {
                description: "Download Docker Desktop for Windows".to_string(),
                command: None,
                url: Some("https://www.docker.com/products/docker-desktop/".to_string()),
            },
            InstallationStep {
                description: "Run installer and follow setup wizard".to_string(),
                command: None,
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("docker --version".to_string()),
                url: None,
            },
        ],
        "darwin" => vec![
            InstallationStep {
                description: "Download Docker Desktop for Mac".to_string(),
                command: None,
                url: Some("https://www.docker.com/products/docker-desktop/".to_string()),
            },
            InstallationStep {
                description: "Install the application".to_string(),
                command: None,
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("docker --version".to_string()),
                url: None,
            },
        ],
        _ => vec![
            InstallationStep {
                description: "Install using package manager".to_string(),
                command: Some("sudo apt-get install docker.io".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Add user to docker group".to_string(),
                command: Some("sudo usermod -aG docker $USER".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("docker --version".to_string()),
                url: None,
            },
        ],
    };

    InstallationGuide {
        tool: "docker".to_string(),
        platform: platform.to_string(),
        steps,
        verification: "docker --version && docker compose version".to_string(),
        notes: vec![
            "Docker Desktop includes Docker Engine, Docker CLI, and Docker Compose".to_string(),
            "You may need to restart your terminal after installation".to_string(),
        ],
    }
}

fn get_git_guide(platform: &str) -> InstallationGuide {
    let steps = match platform {
        "windows" => vec![
            InstallationStep {
                description: "Download Git for Windows".to_string(),
                command: None,
                url: Some("https://git-scm.com/download/win".to_string()),
            },
            InstallationStep {
                description: "Run installer with default options".to_string(),
                command: None,
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("git --version".to_string()),
                url: None,
            },
        ],
        "darwin" => vec![
            InstallationStep {
                description: "Install using Homebrew".to_string(),
                command: Some("brew install git".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Or install Xcode Command Line Tools".to_string(),
                command: Some("xcode-select --install".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("git --version".to_string()),
                url: None,
            },
        ],
        _ => vec![
            InstallationStep {
                description: "Install using package manager".to_string(),
                command: Some("sudo apt-get install git".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Verify installation".to_string(),
                command: Some("git --version".to_string()),
                url: None,
            },
        ],
    };

    InstallationGuide {
        tool: "git".to_string(),
        platform: platform.to_string(),
        steps,
        verification: "git --version".to_string(),
        notes: vec![
            "After installation, configure Git with your name and email".to_string(),
            "Use: git config --global user.name \"Your Name\"".to_string(),
            "Use: git config --global user.email \"your.email@example.com\"".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_node_guide() {
        let guide = get_installation_guide("node", Some("windows"));
        assert_eq!(guide.tool, "node");
        assert_eq!(guide.platform, "windows");
        assert!(!guide.steps.is_empty());
        assert!(!guide.verification.is_empty());
    }

    #[test]
    fn test_get_python_guide() {
        let guide = get_installation_guide("python", Some("darwin"));
        assert_eq!(guide.tool, "python");
        assert!(!guide.steps.is_empty());
    }

    #[test]
    fn test_unknown_tool() {
        let guide = get_installation_guide("unknown", Some("linux"));
        assert!(guide.steps.is_empty());
        assert!(!guide.notes.is_empty());
    }

    #[test]
    fn test_auto_platform_detection() {
        let guide = get_installation_guide("git", None);
        assert_eq!(guide.tool, "git");
        assert!(!guide.platform.is_empty());
    }
}
