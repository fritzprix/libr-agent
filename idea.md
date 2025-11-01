# Bootstrap Built-in Tools

## 🎯 Overview

MCP server 통합을 위해 시스템에는 node / uv / python / docker 등의 의존성이 필요하다.
AI Agent가 자동으로 이러한 도구들을 설치할 수 있도록 Bootstrap 가이드를 제공하는 Built-in MCP Server를 구현한다.

## 🏗️ Architecture

### Server Type: **Web Worker Backend**

- **이유**:
  - 플랫폼 감지는 브라우저 API로 충분 (`navigator.platform`, `navigator.userAgent`)
  - 설치 명령어는 정적 가이드이므로 Rust 구현 불필요
  - Web search를 활용한 동적 가이드 생성 가능
  - 다른 builtin tool (`execute_shell`, `execute_windows_cmd`) 사용 가능

### Location

- `src/lib/web-mcp/modules/bootstrap-server/`

## 🔧 Tool Specifications

### 1. `detect_platform`

AI Agent가 현재 실행 환경을 확인할 수 있는 도구

**Input Schema:**

```typescript
{
  type: 'object',
  properties: {},
  required: []
}
```

**Output:**

```json
{
  "platform": "windows" | "linux" | "darwin",
  "arch": "x64" | "arm64" | "arm" | "ia32",
  "shell": "powershell" | "cmd" | "bash" | "sh" | "zsh",
  "os_details": {
    "type": "Windows_NT" | "Linux" | "Darwin",
    "release": "10.0.19044",
    "version": "#2251"
  }
}
```

### 2. `get_bootstrap_guide`

특정 도구의 설치 가이드를 반환

**Input Schema:**

```typescript
{
  type: 'object',
  properties: {
    tool: {
      type: 'string',
      enum: ['node', 'python', 'uv', 'docker', 'git'],
      description: 'The tool to install'
    },
    platform: {
      type: 'string',
      enum: ['windows', 'linux', 'darwin', 'auto'],
      description: 'Target platform (default: auto-detect)'
    },
    method: {
      type: 'string',
      enum: ['package_manager', 'installer', 'portable', 'all'],
      description: 'Installation method preference (default: all)'
    }
  },
  required: ['tool']
}
```

**Output Example (Node.js on Windows):**

```json
{
  "tool": "node",
  "platform": "windows",
  "methods": [
    {
      "name": "winget",
      "description": "Install via Windows Package Manager",
      "prerequisites": ["winget"],
      "steps": [
        {
          "description": "Install Node.js LTS",
          "command": "winget install -e --id OpenJS.NodeJS.LTS",
          "shell": "powershell"
        },
        {
          "description": "Verify installation",
          "command": "node --version",
          "shell": "powershell"
        }
      ],
      "recommended": true
    },
    {
      "name": "chocolatey",
      "description": "Install via Chocolatey",
      "prerequisites": ["choco"],
      "steps": [
        {
          "description": "Install Node.js",
          "command": "choco install nodejs-lts -y",
          "shell": "powershell"
        }
      ]
    },
    {
      "name": "installer",
      "description": "Download and run official installer",
      "prerequisites": [],
      "steps": [
        {
          "description": "Visit nodejs.org and download Windows installer",
          "url": "https://nodejs.org/en/download/",
          "manual": true
        }
      ]
    }
  ],
  "verification": {
    "command": "node --version && npm --version",
    "shell": "powershell",
    "expected_output_pattern": "v\\d+\\.\\d+\\.\\d+"
  }
}
```

### 3. `check_tool_installed`

도구가 이미 설치되어 있는지 확인

**Input Schema:**

```typescript
{
  type: 'object',
  properties: {
    tool: {
      type: 'string',
      description: 'Tool name or command to check'
    }
  },
  required: ['tool']
}
```

**Output:**

```json
{
  "tool": "node",
  "installed": true,
  "version": "v20.10.0",
  "path": "C:\\Program Files\\nodejs\\node.exe",
  "check_command": "node --version"
}
```

**Implementation:**

```typescript
// Uses existing builtin tools:
// - Windows: execute_windows_cmd
// - Unix: execute_shell
```

## 📋 Installation Guides Database

### Static Guide Templates

```typescript
// src/lib/web-mcp/modules/bootstrap-server/guides.ts

export const BOOTSTRAP_GUIDES = {
  node: {
    windows: {
      winget: {
        name: 'Windows Package Manager',
        commands: ['winget install -e --id OpenJS.NodeJS.LTS'],
        prerequisites: ['winget'],
        recommended: true,
      },
      chocolatey: {
        name: 'Chocolatey',
        commands: ['choco install nodejs-lts -y'],
        prerequisites: ['choco'],
      },
      installer: {
        name: 'Official Installer',
        url: 'https://nodejs.org/en/download/',
        manual: true,
      },
    },
    linux: {
      apt: {
        name: 'APT (Debian/Ubuntu)',
        commands: [
          'curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -',
          'sudo apt-get install -y nodejs',
        ],
        prerequisites: ['curl', 'sudo'],
        recommended: true,
      },
      snap: {
        name: 'Snap',
        commands: ['sudo snap install node --classic'],
        prerequisites: ['snap', 'sudo'],
      },
      nvm: {
        name: 'Node Version Manager',
        commands: [
          'curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash',
          'source ~/.bashrc',
          'nvm install --lts',
        ],
        prerequisites: ['curl'],
      },
    },
    darwin: {
      homebrew: {
        name: 'Homebrew',
        commands: ['brew install node'],
        prerequisites: ['brew'],
        recommended: true,
      },
      nvm: {
        name: 'Node Version Manager',
        commands: [
          'curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash',
          'source ~/.zshrc',
          'nvm install --lts',
        ],
        prerequisites: ['curl'],
      },
    },
  },

  python: {
    windows: {
      winget: {
        name: 'Windows Package Manager',
        commands: ['winget install -e --id Python.Python.3.12'],
        recommended: true,
      },
      chocolatey: {
        commands: ['choco install python -y'],
      },
      installer: {
        url: 'https://www.python.org/downloads/',
        manual: true,
      },
    },
    linux: {
      apt: {
        commands: [
          'sudo apt-get update',
          'sudo apt-get install -y python3 python3-pip',
        ],
        recommended: true,
      },
      dnf: {
        commands: ['sudo dnf install -y python3 python3-pip'],
      },
    },
    darwin: {
      homebrew: {
        commands: ['brew install python@3.12'],
        recommended: true,
      },
    },
  },

  uv: {
    windows: {
      powershell: {
        name: 'PowerShell Install Script',
        commands: ['irm https://astral.sh/uv/install.ps1 | iex'],
        recommended: true,
      },
    },
    linux: {
      curl: {
        commands: ['curl -LsSf https://astral.sh/uv/install.sh | sh'],
        recommended: true,
      },
    },
    darwin: {
      homebrew: {
        commands: ['brew install uv'],
        recommended: true,
      },
      curl: {
        commands: ['curl -LsSf https://astral.sh/uv/install.sh | sh'],
      },
    },
  },

  docker: {
    windows: {
      installer: {
        name: 'Docker Desktop for Windows',
        url: 'https://www.docker.com/products/docker-desktop/',
        notes:
          'Requires Windows 10/11 Pro, Enterprise, or Education with Hyper-V',
        manual: true,
        recommended: true,
      },
      chocolatey: {
        commands: ['choco install docker-desktop -y'],
      },
    },
    linux: {
      apt: {
        name: 'Docker Engine (Ubuntu/Debian)',
        commands: [
          'sudo apt-get update',
          'sudo apt-get install -y ca-certificates curl',
          'sudo install -m 0755 -d /etc/apt/keyrings',
          'sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc',
          'sudo chmod a+r /etc/apt/keyrings/docker.asc',
          'echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null',
          'sudo apt-get update',
          'sudo apt-get install -y docker-ce docker-ce-cli containerd.io',
          'sudo systemctl start docker',
          'sudo systemctl enable docker',
          'sudo usermod -aG docker $USER',
        ],
        recommended: true,
      },
    },
    darwin: {
      installer: {
        name: 'Docker Desktop for Mac',
        url: 'https://www.docker.com/products/docker-desktop/',
        manual: true,
        recommended: true,
      },
      homebrew: {
        commands: ['brew install --cask docker'],
      },
    },
  },

  git: {
    windows: {
      winget: {
        commands: ['winget install -e --id Git.Git'],
        recommended: true,
      },
      chocolatey: {
        commands: ['choco install git -y'],
      },
      installer: {
        url: 'https://git-scm.com/download/win',
        manual: true,
      },
    },
    linux: {
      apt: {
        commands: ['sudo apt-get install -y git'],
        recommended: true,
      },
      dnf: {
        commands: ['sudo dnf install -y git'],
      },
    },
    darwin: {
      xcode: {
        commands: ['xcode-select --install'],
        recommended: true,
      },
      homebrew: {
        commands: ['brew install git'],
      },
    },
  },
};
```

## 🔄 AI Agent Workflow

```typescript
// Example AI Agent workflow for bootstrapping Node.js

// 1. Detect platform
const platform = await callTool('bootstrap', 'detect_platform', {});
// Result: { platform: 'windows', shell: 'powershell' }

// 2. Check if already installed
const nodeCheck = await callTool('bootstrap', 'check_tool_installed', {
  tool: 'node',
});
if (nodeCheck.installed) {
  return `Node.js already installed: ${nodeCheck.version}`;
}

// 3. Get installation guide
const guide = await callTool('bootstrap', 'get_bootstrap_guide', {
  tool: 'node',
  platform: 'auto', // Uses detected platform
});

// 4. Execute installation using recommended method
const recommendedMethod = guide.methods.find((m) => m.recommended);
for (const step of recommendedMethod.steps) {
  if (step.manual) {
    // Request user action
    await callTool('ui', 'prompt_user', {
      prompt: `Please ${step.description}: ${step.url}`,
      type: 'text',
    });
  } else {
    // Execute command using builtin tool
    const toolName =
      platform.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell';

    const result = await callTool('workspace', toolName, {
      command: step.command,
      timeout: 300, // 5 minutes for installations
    });

    if (result.exit_code !== 0) {
      throw new Error(`Installation failed: ${result.stderr}`);
    }
  }
}

// 5. Verify installation
const verification = await callTool(
  'workspace',
  platform.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell',
  { command: guide.verification.command },
);

if (verification.exit_code === 0) {
  return `✅ Node.js installed successfully: ${verification.stdout}`;
}
```

## 🎨 Implementation Structure

```
src/lib/web-mcp/modules/bootstrap-server/
├── index.ts              # Export server
├── server.ts             # WebMCPServer implementation
├── tools.ts              # Tool schemas
├── guides.ts             # Static installation guides
├── platform-detector.ts  # Platform detection logic
└── templates/
    └── guide-display.hbs # Optional: HTML template for guide display
```

## ✅ Success Criteria

1. **Platform Detection**
   - ✅ 정확한 OS 감지 (Windows/Linux/macOS)
   - ✅ 아키텍처 감지 (x64/arm64)
   - ✅ 기본 셸 감지

2. **Guide Accuracy**
   - ✅ 각 플랫폼별 최신 설치 방법 제공
   - ✅ 권장 방법 명시
   - ✅ Prerequisites 체크

3. **AI Agent Integration**
   - ✅ AI가 가이드를 파싱하여 자동 실행 가능
   - ✅ `execute_shell`/`execute_windows_cmd`와 연동
   - ✅ 설치 검증 자동화

4. **User Experience**
   - ✅ 수동 설치가 필요한 경우 명확한 안내
   - ✅ UI Tools와 연동하여 사용자 확인 요청
   - ✅ 설치 진행 상황 피드백

## 🚀 Future Enhancements

1. **Dynamic Web Search Integration**
   - Web search MCP를 활용하여 최신 설치 가이드 검색
   - 버전별 설치 방법 자동 업데이트

2. **Dependency Graph**
   - 도구 간 의존성 자동 해결
   - 예: Docker → WSL2 (Windows), uv → Python

3. **Version Management**
   - 특정 버전 설치 가이드
   - 버전 업그레이드/다운그레이드 지원

4. **Post-Installation Setup**
   - 환경변수 설정 가이드
   - 초기 설정 스크립트 제공
