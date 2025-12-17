/**
 * Bootstrap Server Tests
 */

import { describe, it, expect } from 'vitest';
import bootstrapServer from '../server';
import { detectPlatform } from '../platform-detector';
import { BOOTSTRAP_GUIDES } from '../guides';

describe('Bootstrap Server', () => {
  describe('Tool Schema', () => {
    it('should export 3 tools', () => {
      expect(bootstrapServer.tools).toHaveLength(3);
    });

    it('should have detectPlatform tool', () => {
      const tool = bootstrapServer.tools.find(
        (t) => t.name === 'detectPlatform',
      );
      expect(tool).toBeDefined();
      expect(tool?.description).toContain('platform');
    });

    it('should have getBootstrapGuide tool', () => {
      const tool = bootstrapServer.tools.find(
        (t) => t.name === 'getBootstrapGuide',
      );
      expect(tool).toBeDefined();
      expect(tool?.inputSchema.required).toContain('tool');
    });

    it('should have checkToolInstalled tool', () => {
      const tool = bootstrapServer.tools.find(
        (t) => t.name === 'checkToolInstalled',
      );
      expect(tool).toBeDefined();
      expect(tool?.inputSchema.required).toContain('tool');
    });
  });

  describe('detectPlatform', () => {
    it('should detect platform without arguments', async () => {
      const response = await bootstrapServer.callTool('detectPlatform', {});

      expect(response).toBeDefined();
      expect(response.content).toBeDefined();
      expect(response.content?.[0]?.type).toBe('text');

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      expect(structuredData).toBeDefined();
      expect(['windows', 'linux', 'darwin']).toContain(
        structuredData.platform,
      );
      expect(['x64', 'arm64', 'arm', 'ia32']).toContain(structuredData.arch);
      expect(['powershell', 'cmd', 'bash', 'sh', 'zsh']).toContain(
        structuredData.shell,
      );
    });

    it('should detect OS details', async () => {
      const response = await bootstrapServer.callTool('detectPlatform', {});
      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;

      expect(structuredData.os_details).toBeDefined();
      const osDetails = structuredData.os_details as Record<string, unknown>;
      expect(osDetails.type).toBeDefined();
      expect(osDetails.version).toBeDefined();
    });
  });

  describe('getBootstrapGuide', () => {
    it('should return error for missing tool parameter', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {});

      expect(response.content?.[0].type).toBe('text');
      if (response.content?.[0].type === 'text') {
        expect(response.content[0].text).toContain('required');
      }
    });

    it('should return error for invalid tool', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'invalid_tool',
      });

      if (response.content?.[0].type === 'text') {
        expect(response.content[0].text).toContain('Invalid tool');
      }
    });

    it('should return guide for Node.js with auto platform', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'node',
        platform: 'auto',
      });

      expect(response.content?.[0].type).toBe('text');
      if (response.content?.[0].type === 'text') {
        expect(response.content[0].text).toContain('Installation Guide');
        expect(response.content[0].text).toContain('node');
      }

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      expect(structuredData).toBeDefined();
      expect(structuredData.tool).toBe('node');
      expect(structuredData.methods).toBeDefined();
      expect(Array.isArray(structuredData.methods)).toBe(true);
    });

    it('should return guide for Python on Windows', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'python',
        platform: 'windows',
      });

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      expect(structuredData.platform).toBe('windows');
      expect(structuredData.tool).toBe('python');

      const methods = structuredData.methods as Array<
        Record<string, unknown>
      >;
      expect(methods.length).toBeGreaterThan(0);

      // Check for Windows-specific methods
      const methodNames = methods.map((m) => m.name);
      expect(methodNames).toContain('winget');
    });

    it('should filter methods by type', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'node',
        platform: 'windows',
        method: 'package_manager',
      });

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      const methods = structuredData.methods as Array<
        Record<string, unknown>
      >;

      // Should only include package managers (winget, chocolatey)
      const methodNames = methods.map((m) => m.name);
      expect(methodNames).toContain('winget');
      expect(methodNames).not.toContain('installer');
    });

    it('should mark recommended methods', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'git',
        platform: 'linux',
      });

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      const methods = structuredData.methods as Array<
        Record<string, unknown>
      >;

      const recommendedMethods = methods.filter((m) => m.recommended);
      expect(recommendedMethods.length).toBeGreaterThan(0);
    });

    it('should include verification command', async () => {
      const response = await bootstrapServer.callTool('getBootstrapGuide', {
        tool: 'docker',
        platform: 'darwin',
      });

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      const verification = structuredData.verification as Record<
        string,
        unknown
      >;

      expect(verification).toBeDefined();
      expect(verification.command).toBeDefined();
      expect(verification.shell).toBeDefined();
    });
  });

  describe('checkToolInstalled', () => {
    it('should return error for missing tool parameter', async () => {
      const response = await bootstrapServer.callTool(
        'checkToolInstalled',
        {},
      );

      if (response.content?.[0].type === 'text') {
        expect(response.content[0].text).toContain('required');
      }
    });

    it('should return check instructions for node', async () => {
      const response = await bootstrapServer.callTool(
        'checkToolInstalled',
        {
          tool: 'node',
        },
      );

      expect(response.content?.[0].type).toBe('text');
      if (response.content?.[0].type === 'text') {
        expect(response.content[0].text).toContain('Check Instructions');
        expect(response.content[0].text).toContain('node');
      }

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      expect(structuredData.tool).toBe('node');
      expect(structuredData.check_command).toBeDefined();
      expect(structuredData.shell_tool).toBeDefined();
      expect(['execute_shell', 'execute_windows_cmd']).toContain(
        structuredData.shell_tool,
      );
    });

    it('should use custom version flag', async () => {
      const response = await bootstrapServer.callTool(
        'checkToolInstalled',
        {
          tool: 'python',
          versionFlag: '-V',
        },
      );

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      const checkCommand = String(structuredData.check_command);
      expect(checkCommand).toContain('python');
      expect(checkCommand).toContain('-V');
    });

    it('should provide usage instructions', async () => {
      const response = await bootstrapServer.callTool(
        'checkToolInstalled',
        {
          tool: 'git',
        },
      );

      const structuredData = response.structuredContent as Record<
        string,
        unknown
      >;
      expect(structuredData.instructions).toBeDefined();

      const instructions = structuredData.instructions as Record<
        string,
        unknown
      >;
      expect(instructions.step1).toBeDefined();
      expect(instructions.step2).toBeDefined();
      expect(instructions.step3).toBeDefined();
    });
  });

  describe('Platform Detector', () => {
    it('should detect valid platform information', () => {
      const info = detectPlatform();

      expect(['windows', 'linux', 'darwin']).toContain(info.platform);
      expect(['x64', 'arm64', 'arm', 'ia32']).toContain(info.arch);
      expect(['powershell', 'cmd', 'bash', 'sh', 'zsh']).toContain(info.shell);
      expect(info.os_details).toBeDefined();
      expect(info.os_details.type).toBeDefined();
    });
  });

  describe('Bootstrap Guides Database', () => {
    it('should have guides for all supported tools', () => {
      const tools = ['node', 'python', 'uv', 'docker', 'git'];

      tools.forEach((tool) => {
        expect(BOOTSTRAP_GUIDES[tool]).toBeDefined();
      });
    });

    it('should have platform-specific guides', () => {
      const platforms = ['windows', 'linux', 'darwin'];

      platforms.forEach((platform) => {
        expect(BOOTSTRAP_GUIDES.node[platform]).toBeDefined();
        expect(BOOTSTRAP_GUIDES.python[platform]).toBeDefined();
        expect(BOOTSTRAP_GUIDES.git[platform]).toBeDefined();
      });
    });

    it('should have valid method structures', () => {
      const guide = BOOTSTRAP_GUIDES.node.windows;

      expect(guide.methods).toBeDefined();
      expect(guide.methods.length).toBeGreaterThan(0);

      guide.methods.forEach((method) => {
        expect(method.name).toBeDefined();
        expect(method.description).toBeDefined();
        expect(method.steps).toBeDefined();
        expect(method.steps.length).toBeGreaterThan(0);

        method.steps.forEach((step) => {
          expect(step.description).toBeDefined();
          // Either command or manual flag should be present
          // Manual steps must have description, and optionally url or notes
          if (!step.manual) {
            expect(step.command).toBeDefined();
          }
          // For manual steps, description is always required (already checked above)
          // url and notes are optional for manual steps
        });
      });
    });

    it('should have verification commands', () => {
      Object.values(BOOTSTRAP_GUIDES).forEach((toolGuides) => {
        Object.values(toolGuides).forEach((guide) => {
          expect(guide.verification).toBeDefined();
          expect(guide.verification.command).toBeDefined();
          expect(guide.verification.shell).toBeDefined();
        });
      });
    });
  });
});
