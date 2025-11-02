/**
 * Bootstrap Server - Built-in MCP server for dependency installation guides
 *
 * Provides tools for:
 * - Detecting current platform and environment
 * - Getting installation guides for development tools (Node.js, Python, uv, Docker, Git)
 * - Checking if tools are already installed
 */

import {
  createMCPStructuredResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, WebMCPServer } from '@/lib/mcp-types';
import { bootstrapToolsSchema } from './tools';
import { detectPlatform, getCheckCommand } from './platform-detector';
import { BOOTSTRAP_GUIDES, type ToolGuide } from './guides';

const bootstrapServer: WebMCPServer = {
  name: 'bootstrap',
  version: '1.0.0',
  description:
    'Built-in server for platform detection and dependency installation guides',
  tools: bootstrapToolsSchema,

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    try {
      switch (name) {
        case 'detect_platform': {
          const platformInfo = detectPlatform();

          return createMCPStructuredResponse(
            `🖥️  Platform Detection Results:\n` +
              `OS: ${platformInfo.platform} (${platformInfo.os_details.type})\n` +
              `Architecture: ${platformInfo.arch}\n` +
              `Default Shell: ${platformInfo.shell}\n` +
              `${platformInfo.os_details.release ? `Version: ${platformInfo.os_details.release}` : ''}`,
            platformInfo,
          );
        }

        case 'get_bootstrap_guide': {
          const tool = String(a.tool || '');
          const platformParam = String(a.platform || 'auto');
          const methodFilter = String(a.method || 'all');

          if (!tool) {
            return createMCPTextResponse('Tool name is required');
          }

          // Validate tool
          const validTools = ['node', 'python', 'uv', 'docker', 'git'];
          if (!validTools.includes(tool)) {
            return createMCPTextResponse(
              `Invalid tool: ${tool}. Valid tools: ${validTools.join(', ')}`,
            );
          }

          // Detect or use provided platform
          let platform: string;
          if (platformParam === 'auto') {
            const detected = detectPlatform();
            platform = detected.platform;
          } else {
            platform = platformParam;
          }

          // Get guide
          const toolGuides = BOOTSTRAP_GUIDES[tool];
          if (!toolGuides || !toolGuides[platform]) {
            return createMCPTextResponse(
              `No installation guide available for ${tool} on ${platform}`,
            );
          }

          const guide = toolGuides[platform];

          // Filter methods if requested
          let methods = guide.methods;
          if (methodFilter !== 'all') {
            if (methodFilter === 'package_manager') {
              methods = methods.filter(
                (m) =>
                  m.name === 'winget' ||
                  m.name === 'chocolatey' ||
                  m.name === 'apt' ||
                  m.name === 'dnf' ||
                  m.name === 'homebrew' ||
                  m.name === 'snap',
              );
            } else if (methodFilter === 'installer') {
              methods = methods.filter((m) => m.name === 'installer');
            } else if (methodFilter === 'portable') {
              methods = methods.filter(
                (m) => m.name === 'curl' || m.name === 'powershell',
              );
            }
          }

          const fullGuide: ToolGuide = {
            tool,
            platform,
            methods,
            verification: guide.verification,
            post_install_notes: guide.post_install_notes,
          };

          // Generate human-readable summary
          const summary = [
            `📦 Installation Guide: ${tool} on ${platform}`,
            ``,
            `Available Methods (${methods.length}):`,
          ];

          methods.forEach((method, idx) => {
            const marker = method.recommended ? '⭐' : '  ';
            summary.push(
              `${marker} ${idx + 1}. ${method.name} - ${method.description}`,
            );

            if (method.prerequisites && method.prerequisites.length > 0) {
              summary.push(
                `     Prerequisites: ${method.prerequisites.join(', ')}`,
              );
            }

            summary.push(`     Steps: ${method.steps.length}`);
            method.steps.forEach((step, stepIdx) => {
              if (step.command) {
                summary.push(`       ${stepIdx + 1}. ${step.description}`);
                summary.push(`          $ ${step.command}`);
              } else if (step.url) {
                summary.push(`       ${stepIdx + 1}. ${step.description}`);
                summary.push(`          URL: ${step.url}`);
              } else {
                summary.push(`       ${stepIdx + 1}. ${step.description}`);
              }
            });
            summary.push('');
          });

          summary.push(`Verification Command: ${guide.verification.command}`);

          if (guide.post_install_notes) {
            summary.push('');
            summary.push('Post-Installation Notes:');
            guide.post_install_notes.forEach((note) => {
              summary.push(`  - ${note}`);
            });
          }

          return createMCPStructuredResponse(summary.join('\n'), fullGuide);
        }

        case 'check_tool_installed': {
          const tool = String(a.tool || '');
          const versionFlag = String(a.versionFlag || '--version');

          if (!tool) {
            return createMCPTextResponse('Tool name is required');
          }

          // Get platform info
          const platformInfo = detectPlatform();
          const checkCommand = getCheckCommand(tool, platformInfo, versionFlag);

          // Return instructions for checking
          // The actual execution should be done by the AI agent using execute_shell/execute_windows_cmd
          return createMCPStructuredResponse(
            `🔍 Tool Check Instructions for "${tool}":\n\n` +
              `To check if ${tool} is installed, execute the following command using ` +
              `the "${platformInfo.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell'}" tool:\n\n` +
              `Command: ${checkCommand}\n\n` +
              `Expected behavior:\n` +
              `- If installed: Shows version information (exit code 0)\n` +
              `- If not installed: Command not found error (exit code != 0)\n\n` +
              `Example usage:\n` +
              `callTool('workspace', '${platformInfo.platform === 'windows' ? 'execute_windows_cmd' : 'execute_shell'}', {\n` +
              `  command: '${checkCommand}',\n` +
              `  timeout: 10\n` +
              `})`,
            {
              tool,
              check_command: checkCommand,
              shell_tool:
                platformInfo.platform === 'windows'
                  ? 'execute_windows_cmd'
                  : 'execute_shell',
              platform: platformInfo.platform,
              instructions: {
                step1: `Call the shell execution tool with command: ${checkCommand}`,
                step2:
                  'Check exit_code in response (0 = installed, non-zero = not installed)',
                step3: 'Parse version from stdout if needed',
              },
            },
          );
        }

        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (err) {
      return createMCPTextResponse(
        `Error in bootstrap server: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  },
};

export default bootstrapServer;
