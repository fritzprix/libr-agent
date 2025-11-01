# Bootstrap Server

Built-in Web MCP server for platform detection and dependency installation guides.

## Overview

The Bootstrap Server helps AI agents automatically detect the current platform and install required development dependencies (Node.js, Python, uv, Docker, Git) by providing detailed, platform-specific installation guides.

## Features

- **Platform Detection**: Automatically detect OS, architecture, and default shell
- **Installation Guides**: Comprehensive installation instructions for common dev tools
- **Multiple Methods**: Support for package managers, official installers, and portable installations
- **Cross-Platform**: Windows, Linux (Debian/Ubuntu, Fedora/RHEL), and macOS support
- **Integration Ready**: Designed to work with `execute_shell` and `execute_windows_cmd` builtin tools

## Tools

### 1. `detect_platform`

Detects the current platform, architecture, and shell environment.

**Parameters**: None

**Returns**:

```json
{
  "platform": "windows" | "linux" | "darwin",
  "arch": "x64" | "arm64" | "arm" | "ia32",
  "shell": "powershell" | "cmd" | "bash" | "sh" | "zsh",
  "os_details": {
    "type": "Windows_NT" | "Linux" | "Darwin",
    "version": "10.0",
    "release": "Windows 10/11"
  }
}
```

### 2. `get_bootstrap_guide`

Get installation guide for a specific development tool.

**Parameters**:

- `tool` (required): `"node" | "python" | "uv" | "docker" | "git"`
- `platform` (optional): `"windows" | "linux" | "darwin" | "auto"` (default: "auto")
- `method` (optional): `"package_manager" | "installer" | "portable" | "all"` (default: "all")

**Returns**: Complete installation guide with methods, steps, and verification commands

### 3. `check_tool_installed`

Get instructions for checking if a tool is already installed.

**Parameters**:

- `tool` (required): Tool name or command to check (e.g., "node", "python3")
- `versionFlag` (optional): Version flag (default: "--version")

**Returns**: Instructions and command for checking tool installation

## Usage Examples

### Basic AI Agent Workflow

```typescript
// 1. Detect platform
const platform = await callTool('bootstrap', 'detect_platform', {});
console.log(platform.platform); // "windows", "linux", or "darwin"

// 2. Check if Node.js is installed
const checkInstructions = await callTool('bootstrap', 'check_tool_installed', {
  tool: 'node',
});

// 3. Execute check using appropriate shell tool
const shellTool =
  platform.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell';

const checkResult = await callTool('workspace', shellTool, {
  command: checkInstructions.structured_data.check_command,
});

if (checkResult.exit_code !== 0) {
  // 4. Get installation guide
  const guide = await callTool('bootstrap', 'get_bootstrap_guide', {
    tool: 'node',
    platform: 'auto',
  });

  // 5. Execute installation using recommended method
  const recommendedMethod = guide.structured_data.methods.find(
    (m) => m.recommended,
  );

  for (const step of recommendedMethod.steps) {
    if (step.manual) {
      // Request user action via UI tools
      await callTool('ui', 'prompt_user', {
        prompt: `${step.description}: ${step.url || step.notes}`,
        type: 'text',
      });
    } else {
      // Execute command
      await callTool('workspace', shellTool, {
        command: step.command,
        timeout: 300,
      });
    }
  }
}
```

### Checking Multiple Tools

```typescript
async function checkDependencies(tools: string[]) {
  const platform = await callTool('bootstrap', 'detect_platform', {});
  const shellTool =
    platform.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell';

  const results = [];

  for (const tool of tools) {
    const checkInstructions = await callTool(
      'bootstrap',
      'check_tool_installed',
      {
        tool,
      },
    );

    const result = await callTool('workspace', shellTool, {
      command: checkInstructions.structured_data.check_command,
    });

    results.push({
      tool,
      installed: result.exit_code === 0,
      version: result.exit_code === 0 ? result.stdout.trim() : null,
    });
  }

  return results;
}

// Usage
const status = await checkDependencies(['node', 'python', 'git', 'docker']);
console.log(status);
// [
//   { tool: 'node', installed: true, version: 'v20.10.0' },
//   { tool: 'python', installed: false, version: null },
//   ...
// ]
```

### Installing with Preferred Method

```typescript
async function installTool(tool: string, preferredMethod?: string) {
  const guide = await callTool('bootstrap', 'get_bootstrap_guide', {
    tool,
    platform: 'auto',
    method: preferredMethod || 'all',
  });

  const methods = guide.structured_data.methods;
  const method = preferredMethod
    ? methods.find((m) => m.name === preferredMethod)
    : methods.find((m) => m.recommended) || methods[0];

  if (!method) {
    throw new Error(`No suitable installation method found for ${tool}`);
  }

  console.log(`Installing ${tool} using ${method.name}...`);

  // Check prerequisites
  if (method.prerequisites) {
    for (const prereq of method.prerequisites) {
      const check = await checkToolInstalled(prereq);
      if (!check.installed) {
        throw new Error(`Prerequisite not found: ${prereq}`);
      }
    }
  }

  // Execute steps
  for (const step of method.steps) {
    if (step.manual) {
      console.log(`Manual step required: ${step.description}`);
      console.log(`URL: ${step.url}`);
      // Wait for user confirmation via UI tools
    } else {
      console.log(`Executing: ${step.command}`);
      const result = await executeShellCommand(step.command);
      if (result.exit_code !== 0) {
        throw new Error(`Installation failed: ${result.stderr}`);
      }
    }
  }

  return guide;
}
```

## Supported Tools

### Node.js

- **Windows**: winget, Chocolatey, official installer
- **Linux**: APT, Snap, NVM
- **macOS**: Homebrew, NVM

### Python

- **Windows**: winget, Chocolatey, official installer
- **Linux**: APT, DNF
- **macOS**: Homebrew

### uv (Python package manager)

- **Windows**: PowerShell script
- **Linux**: curl script
- **macOS**: Homebrew, curl script

### Docker

- **Windows**: Docker Desktop
- **Linux**: Docker Engine (APT)
- **macOS**: Docker Desktop, Homebrew

### Git

- **Windows**: winget, Chocolatey, official installer
- **Linux**: APT, DNF
- **macOS**: Xcode Command Line Tools, Homebrew

## Architecture

### Platform Detection

Uses browser APIs to detect:

- Operating System (Windows/Linux/macOS)
- Architecture (x64/arm64/arm/ia32)
- Default Shell (PowerShell/cmd/bash/sh/zsh)

### Installation Guides

Static database of installation methods:

- **Package Managers**: winget, Chocolatey, APT, DNF, Homebrew, Snap
- **Official Installers**: Manual download and installation
- **Portable Scripts**: curl, PowerShell scripts

### Integration with Other Tools

Bootstrap server is designed to work seamlessly with:

- **workspace server**: `execute_shell`, `execute_windows_cmd` for command execution
- **ui server**: `prompt_user` for manual installation steps
- **Web search MCP**: Future integration for dynamic guide updates

## Best Practices

1. **Always detect platform first**: Use `detect_platform` before requesting guides
2. **Check before installing**: Use `check_tool_installed` to avoid redundant installations
3. **Use recommended methods**: Prioritize methods marked as `recommended: true`
4. **Handle manual steps**: Some installations require user interaction (Docker Desktop, etc.)
5. **Verify after installation**: Always run verification commands
6. **Set appropriate timeouts**: Installation commands may take several minutes

## Limitations

1. **Browser-based detection**: May not detect all OS variants accurately
2. **Static guides**: Installation methods may become outdated over time
3. **No actual execution**: This server only provides guides; execution must be done via shell tools
4. **Limited version control**: Cannot specify exact versions for most tools

## Future Enhancements

1. **Web Search Integration**: Fetch latest installation guides dynamically
2. **Version Specification**: Support for installing specific versions
3. **Dependency Resolution**: Automatic detection and installation of prerequisites
4. **Post-Install Configuration**: Scripts for environment setup and PATH configuration
5. **Update Detection**: Check for available updates of installed tools
