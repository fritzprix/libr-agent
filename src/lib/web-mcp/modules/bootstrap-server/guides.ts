/**
 * Bootstrap Installation Guides Database
 *
 * Static database of installation instructions for common development tools
 */

export interface InstallationStep {
  description: string;
  command?: string;
  shell?: 'powershell' | 'cmd' | 'bash' | 'sh' | 'zsh';
  url?: string;
  manual?: boolean;
  notes?: string;
}

export interface InstallationMethod {
  name: string;
  description: string;
  prerequisites?: string[];
  steps: InstallationStep[];
  recommended?: boolean;
  notes?: string;
}

export interface ToolGuide {
  tool: string;
  platform: string;
  methods: InstallationMethod[];
  verification: {
    command: string;
    shell: string;
    expected_output_pattern?: string;
  };
  post_install_notes?: string[];
}

export const BOOTSTRAP_GUIDES: Record<
  string,
  Record<string, Omit<ToolGuide, 'tool' | 'platform'>>
> = {
  node: {
    windows: {
      methods: [
        {
          name: 'winget',
          description: 'Install via Windows Package Manager (recommended)',
          prerequisites: ['winget'],
          recommended: true,
          steps: [
            {
              description: 'Install Node.js LTS using winget',
              command: 'winget install -e --id OpenJS.NodeJS.LTS',
              shell: 'powershell',
            },
            {
              description: 'Verify Node.js installation',
              command: 'node --version',
              shell: 'powershell',
            },
            {
              description: 'Verify npm installation',
              command: 'npm --version',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'chocolatey',
          description: 'Install via Chocolatey package manager',
          prerequisites: ['choco'],
          steps: [
            {
              description: 'Install Node.js using Chocolatey',
              command: 'choco install nodejs-lts -y',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'installer',
          description: 'Download and run official installer (manual)',
          prerequisites: [],
          steps: [
            {
              description: 'Download Windows installer from nodejs.org',
              url: 'https://nodejs.org/en/download/',
              manual: true,
              notes:
                'Choose the Windows Installer (.msi) for your architecture (x64 or x86)',
            },
            {
              description: 'Run the downloaded installer and follow the wizard',
              manual: true,
            },
          ],
        },
      ],
      verification: {
        command: 'node --version && npm --version',
        shell: 'powershell',
        expected_output_pattern: 'v\\d+\\.\\d+\\.\\d+',
      },
      post_install_notes: [
        'Restart your terminal/command prompt to update PATH',
        'Run "npm config set prefix %APPDATA%\\npm" to set global install directory',
      ],
    },
    linux: {
      methods: [
        {
          name: 'apt',
          description: 'Install via APT (Debian/Ubuntu - recommended)',
          prerequisites: ['curl', 'sudo'],
          recommended: true,
          steps: [
            {
              description: 'Add NodeSource repository',
              command:
                'curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -',
              shell: 'bash',
            },
            {
              description: 'Install Node.js',
              command: 'sudo apt-get install -y nodejs',
              shell: 'bash',
            },
          ],
        },
        {
          name: 'snap',
          description: 'Install via Snap package manager',
          prerequisites: ['snap', 'sudo'],
          steps: [
            {
              description: 'Install Node.js using Snap',
              command: 'sudo snap install node --classic',
              shell: 'bash',
            },
          ],
        },
        {
          name: 'nvm',
          description: 'Install via Node Version Manager (flexible)',
          prerequisites: ['curl'],
          steps: [
            {
              description: 'Download and install nvm',
              command:
                'curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash',
              shell: 'bash',
            },
            {
              description: 'Reload shell configuration',
              command: 'source ~/.bashrc',
              shell: 'bash',
            },
            {
              description: 'Install Node.js LTS',
              command: 'nvm install --lts',
              shell: 'bash',
            },
          ],
        },
      ],
      verification: {
        command: 'node --version && npm --version',
        shell: 'bash',
        expected_output_pattern: 'v\\d+\\.\\d+\\.\\d+',
      },
    },
    darwin: {
      methods: [
        {
          name: 'homebrew',
          description: 'Install via Homebrew (recommended)',
          prerequisites: ['brew'],
          recommended: true,
          steps: [
            {
              description: 'Install Node.js using Homebrew',
              command: 'brew install node',
              shell: 'zsh',
            },
          ],
        },
        {
          name: 'nvm',
          description: 'Install via Node Version Manager',
          prerequisites: ['curl'],
          steps: [
            {
              description: 'Download and install nvm',
              command:
                'curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash',
              shell: 'zsh',
            },
            {
              description: 'Reload shell configuration',
              command: 'source ~/.zshrc',
              shell: 'zsh',
            },
            {
              description: 'Install Node.js LTS',
              command: 'nvm install --lts',
              shell: 'zsh',
            },
          ],
        },
      ],
      verification: {
        command: 'node --version && npm --version',
        shell: 'zsh',
        expected_output_pattern: 'v\\d+\\.\\d+\\.\\d+',
      },
    },
  },

  python: {
    windows: {
      methods: [
        {
          name: 'winget',
          description: 'Install via Windows Package Manager (recommended)',
          prerequisites: ['winget'],
          recommended: true,
          steps: [
            {
              description: 'Install Python 3.12',
              command: 'winget install -e --id Python.Python.3.12',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'chocolatey',
          description: 'Install via Chocolatey',
          prerequisites: ['choco'],
          steps: [
            {
              description: 'Install Python',
              command: 'choco install python -y',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'installer',
          description: 'Download official installer',
          prerequisites: [],
          steps: [
            {
              description: 'Download Python installer',
              url: 'https://www.python.org/downloads/',
              manual: true,
              notes:
                'Make sure to check "Add Python to PATH" during installation',
            },
          ],
        },
      ],
      verification: {
        command: 'python --version && pip --version',
        shell: 'powershell',
        expected_output_pattern: 'Python \\d+\\.\\d+\\.\\d+',
      },
      post_install_notes: [
        'Restart terminal to update PATH',
        'Verify pip is working: pip --version',
      ],
    },
    linux: {
      methods: [
        {
          name: 'apt',
          description: 'Install via APT (Debian/Ubuntu - recommended)',
          prerequisites: ['sudo'],
          recommended: true,
          steps: [
            {
              description: 'Update package list',
              command: 'sudo apt-get update',
              shell: 'bash',
            },
            {
              description: 'Install Python 3 and pip',
              command:
                'sudo apt-get install -y python3 python3-pip python3-venv',
              shell: 'bash',
            },
          ],
        },
        {
          name: 'dnf',
          description: 'Install via DNF (Fedora/RHEL)',
          prerequisites: ['sudo'],
          steps: [
            {
              description: 'Install Python 3 and pip',
              command: 'sudo dnf install -y python3 python3-pip',
              shell: 'bash',
            },
          ],
        },
      ],
      verification: {
        command: 'python3 --version && pip3 --version',
        shell: 'bash',
        expected_output_pattern: 'Python \\d+\\.\\d+\\.\\d+',
      },
    },
    darwin: {
      methods: [
        {
          name: 'homebrew',
          description: 'Install via Homebrew (recommended)',
          prerequisites: ['brew'],
          recommended: true,
          steps: [
            {
              description: 'Install Python 3.12',
              command: 'brew install python@3.12',
              shell: 'zsh',
            },
          ],
        },
      ],
      verification: {
        command: 'python3 --version && pip3 --version',
        shell: 'zsh',
        expected_output_pattern: 'Python \\d+\\.\\d+\\.\\d+',
      },
    },
  },

  uv: {
    windows: {
      methods: [
        {
          name: 'powershell',
          description: 'Install via PowerShell script (recommended)',
          prerequisites: [],
          recommended: true,
          steps: [
            {
              description: 'Download and run uv installer',
              command: 'irm https://astral.sh/uv/install.ps1 | iex',
              shell: 'powershell',
            },
          ],
        },
      ],
      verification: {
        command: 'uv --version',
        shell: 'powershell',
        expected_output_pattern: 'uv \\d+\\.\\d+\\.\\d+',
      },
      post_install_notes: [
        'Restart terminal to update PATH',
        'uv is installed to %USERPROFILE%\\.cargo\\bin',
      ],
    },
    linux: {
      methods: [
        {
          name: 'curl',
          description: 'Install via shell script (recommended)',
          prerequisites: ['curl'],
          recommended: true,
          steps: [
            {
              description: 'Download and run uv installer',
              command: 'curl -LsSf https://astral.sh/uv/install.sh | sh',
              shell: 'bash',
            },
          ],
        },
      ],
      verification: {
        command: 'uv --version',
        shell: 'bash',
        expected_output_pattern: 'uv \\d+\\.\\d+\\.\\d+',
      },
      post_install_notes: [
        'Restart terminal or source ~/.bashrc to update PATH',
        'uv is installed to ~/.cargo/bin',
      ],
    },
    darwin: {
      methods: [
        {
          name: 'homebrew',
          description: 'Install via Homebrew (recommended)',
          prerequisites: ['brew'],
          recommended: true,
          steps: [
            {
              description: 'Install uv',
              command: 'brew install uv',
              shell: 'zsh',
            },
          ],
        },
        {
          name: 'curl',
          description: 'Install via shell script',
          prerequisites: ['curl'],
          steps: [
            {
              description: 'Download and run uv installer',
              command: 'curl -LsSf https://astral.sh/uv/install.sh | sh',
              shell: 'zsh',
            },
          ],
        },
      ],
      verification: {
        command: 'uv --version',
        shell: 'zsh',
        expected_output_pattern: 'uv \\d+\\.\\d+\\.\\d+',
      },
    },
  },

  git: {
    windows: {
      methods: [
        {
          name: 'winget',
          description: 'Install via Windows Package Manager (recommended)',
          prerequisites: ['winget'],
          recommended: true,
          steps: [
            {
              description: 'Install Git',
              command: 'winget install -e --id Git.Git',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'chocolatey',
          description: 'Install via Chocolatey',
          prerequisites: ['choco'],
          steps: [
            {
              description: 'Install Git',
              command: 'choco install git -y',
              shell: 'powershell',
            },
          ],
        },
        {
          name: 'installer',
          description: 'Download official installer',
          prerequisites: [],
          steps: [
            {
              description: 'Download Git installer',
              url: 'https://git-scm.com/download/win',
              manual: true,
            },
          ],
        },
      ],
      verification: {
        command: 'git --version',
        shell: 'powershell',
        expected_output_pattern: 'git version \\d+\\.\\d+',
      },
    },
    linux: {
      methods: [
        {
          name: 'apt',
          description: 'Install via APT (Debian/Ubuntu - recommended)',
          prerequisites: ['sudo'],
          recommended: true,
          steps: [
            {
              description: 'Install Git',
              command: 'sudo apt-get install -y git',
              shell: 'bash',
            },
          ],
        },
        {
          name: 'dnf',
          description: 'Install via DNF (Fedora/RHEL)',
          prerequisites: ['sudo'],
          steps: [
            {
              description: 'Install Git',
              command: 'sudo dnf install -y git',
              shell: 'bash',
            },
          ],
        },
      ],
      verification: {
        command: 'git --version',
        shell: 'bash',
        expected_output_pattern: 'git version \\d+\\.\\d+',
      },
    },
    darwin: {
      methods: [
        {
          name: 'xcode',
          description: 'Install via Xcode Command Line Tools (recommended)',
          prerequisites: [],
          recommended: true,
          steps: [
            {
              description: 'Install Xcode Command Line Tools (includes Git)',
              command: 'xcode-select --install',
              shell: 'zsh',
              notes: 'This will open a dialog box for installation',
            },
          ],
        },
        {
          name: 'homebrew',
          description: 'Install via Homebrew',
          prerequisites: ['brew'],
          steps: [
            {
              description: 'Install Git',
              command: 'brew install git',
              shell: 'zsh',
            },
          ],
        },
      ],
      verification: {
        command: 'git --version',
        shell: 'zsh',
        expected_output_pattern: 'git version \\d+\\.\\d+',
      },
    },
  },

  docker: {
    windows: {
      methods: [
        {
          name: 'installer',
          description: 'Install Docker Desktop (recommended)',
          prerequisites: [],
          recommended: true,
          steps: [
            {
              description: 'Download Docker Desktop installer',
              url: 'https://www.docker.com/products/docker-desktop/',
              manual: true,
              notes:
                'Requires Windows 10/11 Pro, Enterprise, or Education with Hyper-V enabled',
            },
            {
              description: 'Run installer and follow the wizard',
              manual: true,
            },
            {
              description: 'Restart your computer after installation',
              manual: true,
            },
          ],
        },
      ],
      verification: {
        command: 'docker --version',
        shell: 'powershell',
        expected_output_pattern: 'Docker version \\d+\\.\\d+',
      },
      post_install_notes: [
        'Docker Desktop must be running for docker commands to work',
        'You may need to enable WSL 2 for better performance',
      ],
    },
    linux: {
      methods: [
        {
          name: 'apt',
          description: 'Install Docker Engine (Ubuntu/Debian - recommended)',
          prerequisites: ['curl', 'sudo'],
          recommended: true,
          steps: [
            {
              description: 'Update apt package index',
              command: 'sudo apt-get update',
              shell: 'bash',
            },
            {
              description: 'Install prerequisites',
              command: 'sudo apt-get install -y ca-certificates curl',
              shell: 'bash',
            },
            {
              description: 'Add Docker GPG key',
              command:
                'sudo install -m 0755 -d /etc/apt/keyrings && sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc && sudo chmod a+r /etc/apt/keyrings/docker.asc',
              shell: 'bash',
            },
            {
              description: 'Add Docker repository',
              command:
                'echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null',
              shell: 'bash',
            },
            {
              description: 'Update apt and install Docker',
              command:
                'sudo apt-get update && sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin',
              shell: 'bash',
            },
            {
              description: 'Add user to docker group',
              command: 'sudo usermod -aG docker $USER',
              shell: 'bash',
              notes: 'Logout and login again for group changes to take effect',
            },
          ],
        },
      ],
      verification: {
        command: 'docker --version',
        shell: 'bash',
        expected_output_pattern: 'Docker version \\d+\\.\\d+',
      },
      post_install_notes: [
        'Logout and login again for docker group permissions',
        'Start Docker: sudo systemctl start docker',
        'Enable on boot: sudo systemctl enable docker',
      ],
    },
    darwin: {
      methods: [
        {
          name: 'installer',
          description: 'Install Docker Desktop (recommended)',
          prerequisites: [],
          recommended: true,
          steps: [
            {
              description: 'Download Docker Desktop for Mac',
              url: 'https://www.docker.com/products/docker-desktop/',
              manual: true,
              notes:
                'Choose the installer for your Mac (Intel or Apple Silicon)',
            },
            {
              description: 'Open the .dmg file and drag Docker to Applications',
              manual: true,
            },
          ],
        },
        {
          name: 'homebrew',
          description: 'Install via Homebrew',
          prerequisites: ['brew'],
          steps: [
            {
              description: 'Install Docker Desktop',
              command: 'brew install --cask docker',
              shell: 'zsh',
            },
          ],
        },
      ],
      verification: {
        command: 'docker --version',
        shell: 'zsh',
        expected_output_pattern: 'Docker version \\d+\\.\\d+',
      },
      post_install_notes: [
        'Start Docker Desktop from Applications',
        'Docker Desktop must be running for docker commands to work',
      ],
    },
  },
};
