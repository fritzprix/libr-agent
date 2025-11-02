/**
 * Platform Detection Utilities
 *
 * Detects current platform, architecture, and shell environment
 */

export interface PlatformInfo {
  platform: 'windows' | 'linux' | 'darwin';
  arch: 'x64' | 'arm64' | 'arm' | 'ia32';
  shell: 'powershell' | 'cmd' | 'bash' | 'sh' | 'zsh';
  os_details: {
    type: string;
    version: string;
    release?: string;
  };
}

/**
 * Detect the current platform from browser environment
 */
export function detectPlatform(): PlatformInfo {
  const userAgent = navigator.userAgent.toLowerCase();
  const platform = navigator.platform.toLowerCase();

  // Detect OS
  let osPlatform: 'windows' | 'linux' | 'darwin';
  let osType: string;

  if (platform.includes('win') || userAgent.includes('windows')) {
    osPlatform = 'windows';
    osType = 'Windows_NT';
  } else if (platform.includes('mac') || userAgent.includes('mac')) {
    osPlatform = 'darwin';
    osType = 'Darwin';
  } else if (
    platform.includes('linux') ||
    userAgent.includes('linux') ||
    userAgent.includes('x11')
  ) {
    osPlatform = 'linux';
    osType = 'Linux';
  } else {
    // Fallback to generic detection
    osPlatform = 'linux';
    osType = 'Unknown';
  }

  // Detect architecture
  let arch: 'x64' | 'arm64' | 'arm' | 'ia32' = 'x64';

  // Check for ARM architecture indicators
  if (userAgent.includes('arm64') || userAgent.includes('aarch64')) {
    arch = 'arm64';
  } else if (userAgent.includes('arm')) {
    arch = 'arm';
  } else if (userAgent.includes('x86_64') || userAgent.includes('x64')) {
    arch = 'x64';
  } else if (userAgent.includes('i686') || userAgent.includes('i386')) {
    arch = 'ia32';
  }

  // Detect default shell
  let shell: 'powershell' | 'cmd' | 'bash' | 'sh' | 'zsh';

  if (osPlatform === 'windows') {
    // Windows: PowerShell is preferred on modern systems
    // Check Windows version from user agent if possible
    if (
      userAgent.includes('windows nt 10') ||
      userAgent.includes('windows nt 11')
    ) {
      shell = 'powershell';
    } else {
      shell = 'cmd';
    }
  } else if (osPlatform === 'darwin') {
    // macOS: zsh is default since Catalina (10.15)
    shell = 'zsh';
  } else {
    // Linux: bash is most common default
    shell = 'bash';
  }

  // Extract version info from user agent if possible
  let version = 'unknown';
  let release = undefined;

  if (osPlatform === 'windows') {
    const match = userAgent.match(/windows nt ([\d.]+)/i);
    if (match) {
      version = match[1];
      // Map to friendly names
      const versionMap: Record<string, string> = {
        '10.0': 'Windows 10/11',
        '6.3': 'Windows 8.1',
        '6.2': 'Windows 8',
        '6.1': 'Windows 7',
      };
      release = versionMap[version] || `Windows ${version}`;
    }
  } else if (osPlatform === 'darwin') {
    const match = userAgent.match(/mac os x ([\d._]+)/i);
    if (match) {
      version = match[1].replace(/_/g, '.');
      release = `macOS ${version}`;
    }
  } else {
    // Linux version detection is limited in browser
    version = 'unknown';
  }

  return {
    platform: osPlatform,
    arch,
    shell,
    os_details: {
      type: osType,
      version,
      release,
    },
  };
}

/**
 * Get the appropriate shell tool name based on platform
 */
export function getShellToolName(platform: PlatformInfo): string {
  return platform.platform === 'windows'
    ? 'execute_windows_cmd'
    : 'execute_shell';
}

/**
 * Check if a command is available on the system
 * Returns the command to check tool availability
 */
export function getCheckCommand(
  tool: string,
  platform: PlatformInfo,
  versionFlag = '--version',
): string {
  if (platform.platform === 'windows') {
    // Windows: use where command
    return `where ${tool} && ${tool} ${versionFlag}`;
  } else {
    // Unix: use which command
    return `which ${tool} && ${tool} ${versionFlag}`;
  }
}
