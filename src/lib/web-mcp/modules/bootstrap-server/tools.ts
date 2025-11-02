/**
 * Bootstrap Server Tool Schemas
 *
 * Provides tool schemas for platform detection and dependency installation guides
 */

import type { MCPTool } from '@/lib/mcp-types';

export const bootstrapToolsSchema: MCPTool[] = [
  {
    name: 'detect_platform',
    description:
      'Detect the current platform, architecture, and shell environment for bootstrap operations',
    inputSchema: {
      type: 'object',
      properties: {},
      required: [],
    },
  },
  {
    name: 'get_bootstrap_guide',
    description:
      'Get installation guide for development tools (Node.js, Python, uv, Docker, Git)',
    inputSchema: {
      type: 'object',
      properties: {
        tool: {
          type: 'string',
          enum: ['node', 'python', 'uv', 'docker', 'git'],
          description: 'The tool to get installation guide for',
        },
        platform: {
          type: 'string',
          enum: ['windows', 'linux', 'darwin', 'auto'],
          description:
            'Target platform (default: auto-detect from current environment)',
        },
        method: {
          type: 'string',
          enum: ['package_manager', 'installer', 'portable', 'all'],
          description:
            'Preferred installation method (default: all available methods)',
        },
      },
      required: ['tool'],
    },
  },
  {
    name: 'check_tool_installed',
    description:
      'Check if a development tool is already installed and get version information',
    inputSchema: {
      type: 'object',
      properties: {
        tool: {
          type: 'string',
          description:
            'Tool name or command to check (e.g., "node", "python3")',
        },
        versionFlag: {
          type: 'string',
          description:
            'Version flag to use (default: "--version", alternatives: "-v", "-version", "version")',
        },
      },
      required: ['tool'],
    },
  },
];
