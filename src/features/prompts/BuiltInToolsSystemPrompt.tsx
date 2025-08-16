import { useCallback, useEffect } from 'react';
import { useChatContext } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useWebMCPServer } from '@/context/WebMCPContext';
import { useBuiltInTools } from '@/context/BuiltInToolContext';
import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';
import { getLogger } from '@/lib/logger';

const logger = getLogger('BuiltInToolsSystemPrompt');

/**
 * System prompt component that dynamically injects information about available tools
 * and attached files into the chat context. This allows the AI to be aware of:
 *
 * 1. Built-in MCP Tools (Tauri-based):
 *    - Filesystem operations (read, write, list)
 *    - Code execution sandbox (Python, TypeScript)
 *
 * 2. Web MCP Tools:
 *    - Content store operations
 *    - File parsing and analysis
 *
 * 3. Attached Files:
 *    - Files uploaded to the current session
 *    - File metadata and previews
 *
 * This component automatically registers a system prompt extension that:
 * - Lists all available tools with usage guidelines
 * - Provides tool capabilities and security constraints
 * - Lists all files attached to the current session
 * - Updates when tools or files change
 * - Handles errors gracefully with fallback messages
 */
export function BuiltInToolsSystemPrompt() {
  const { registerSystemPrompt, unregisterSystemPrompt } = useChatContext();
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');
  const { getCurrentSession } = useSessionContext();
  const {
    availableTools,
    tauriBuiltinTools,
    webWorkerTools,
    isLoadingTauriTools,
  } = useBuiltInTools();

  const buildPrompt = useCallback(async () => {
    let promptSections = [];

    // 1. Built-in Tools Section
    promptSections.push(`# Available Built-in Tools

You have access to powerful built-in tools that can help you assist users with various tasks. These tools are secure, fast, and require no external setup.

## 🔧 Filesystem Tools (builtin.filesystem)

**Available Operations:**
- \`builtin.filesystem__read_file\` - Read file contents safely
- \`builtin.filesystem__write_file\` - Write content to files (creates directories as needed)
- \`builtin.filesystem__list_directory\` - List directory contents with metadata

**Security Features:**
- Access restricted to current working directory and subdirectories
- Path validation prevents directory traversal attacks
- File size limits (10MB max) for safety
- Automatic parent directory creation for write operations

**Usage Guidelines:**
- Always use relative paths or paths within the working directory
- Check file sizes before reading large files
- Use list_directory to explore the filesystem structure
- Prefer these tools over asking users to manually provide file contents

**Example Usage:**
\`\`\`
// Read package.json to understand project structure
builtin.filesystem__read_file: {"path": "package.json"}

// List files in src directory
builtin.filesystem__list_directory: {"path": "src"}

// Write a new configuration file
builtin.filesystem__write_file: {"path": "config/new-settings.json", "content": "{\\"setting\\": \\"value\\"}"}
\`\`\`

## 🐍 Code Execution Tools (builtin.sandbox)

**Available Runtimes:**
- \`builtin.sandbox__execute_python\` - Execute Python code in isolated environment
- \`builtin.sandbox__execute_typescript\` - Execute TypeScript/JavaScript code

**Security Features:**
- Isolated temporary directory execution
- Environment variable isolation
- Execution timeout limits (1-60 seconds, default 30)
- Code size limits (10KB max)
- Automatic process cleanup

**Usage Guidelines:**
- Use for data analysis, calculations, demonstrations, and prototyping
- Keep code concise and focused (10KB limit)
- Set appropriate timeout based on expected execution time
- Handle errors gracefully in your code
- Great for showing examples or solving computational problems

**Example Usage:**
\`\`\`
// Data analysis with Python
builtin.sandbox__execute_python: {
  "code": "import json\\ndata = [1,2,3,4,5]\\nprint(f'Average: {sum(data)/len(data)}')",
  "timeout": 10
}

// TypeScript calculations
builtin.sandbox__execute_typescript: {
  "code": "const nums = [1,2,3,4,5];\\nconsole.log('Sum:', nums.reduce((a,b) => a+b, 0));",
  "timeout": 5
}
\`\`\`

## 📊 Web MCP Tools

${
  webWorkerTools.length > 0
    ? `**Available Web Tools:**
${webWorkerTools.map((tool) => `- \`${tool.name}\` - ${tool.description}`).join('\n')}`
    : 'No Web MCP tools currently available.'
}

**Total Available Tools:** ${availableTools.length} ${isLoadingTauriTools ? '(Loading additional tools...)' : ''}
**Built-in Tools:** ${tauriBuiltinTools.length}
**Web Tools:** ${webWorkerTools.length}

## 🎯 Best Practices

1. **Choose the Right Tool:**
   - Use filesystem tools for file operations and project exploration
   - Use sandbox tools for code execution, calculations, and demonstrations
   - Use web tools for specialized processing tasks

2. **Be Proactive:**
   - Read relevant files to understand user's project structure
   - Use list_directory to explore and understand the codebase
   - Execute code to demonstrate concepts or solve problems
   - Write files when creating examples or configurations

3. **Handle Errors Gracefully:**
   - Check if files exist before reading
   - Validate paths and parameters
   - Provide clear error messages and alternatives

4. **Respect Limits:**
   - Keep code execution under timeout limits
   - Don't read unnecessarily large files
   - Use appropriate tools for each task

Remember: These tools are designed to make you more helpful and capable. Use them proactively to provide better assistance!`);

    // 2. Attached Files Section
    const currentSession = getCurrentSession();
    if (!currentSession?.storeId) {
      promptSections.push('\n# Attached Files\nNo files currently attached.');
    } else {
      try {
        const result = await server?.listContent({
          storeId: currentSession.storeId,
        });

        if (!result?.contents || result.contents.length === 0) {
          promptSections.push(
            '\n# Attached Files\nNo files currently attached.',
          );
        } else {
          const attachedResources = result.contents
            .map((c) =>
              JSON.stringify({
                storeId: c.storeId,
                contentId: c.contentId,
                preview: c.preview,
                filename: c.filename,
                type: c.mimeType,
                size: c.size,
              }),
            )
            .join('\n');

          promptSections.push(`\n# Attached Files\n${attachedResources}`);

          logger.debug('Built attached files prompt', {
            sessionId: currentSession.id,
            fileCount: result.contents.length,
          });
        }
      } catch (error) {
        logger.error('Failed to build attached files prompt', {
          sessionId: currentSession.id,
          error: error instanceof Error ? error.message : String(error),
        });
        promptSections.push(
          '\n# Attached Files\nError loading attached files.',
        );
      }
    }

    const fullPrompt = promptSections.join('\n');

    logger.debug('Built comprehensive tools and files prompt', {
      sessionId: currentSession?.id,
      toolCount: availableTools.length,
      tauriToolCount: tauriBuiltinTools.length,
      webToolCount: webWorkerTools.length,
      promptLength: fullPrompt.length,
    });

    return fullPrompt;
  }, [
    getCurrentSession,
    server,
    availableTools,
    tauriBuiltinTools,
    webWorkerTools,
    isLoadingTauriTools,
  ]);

  useEffect(() => {
    // Register the system prompt even if server is not available yet
    // Built-in tools should always be available
    const id = registerSystemPrompt({
      content: buildPrompt,
      priority: 1,
    });

    logger.debug('Registered built-in tools system prompt', {
      promptId: id,
      toolCount: availableTools.length,
      hasServer: !!server,
    });

    return () => {
      unregisterSystemPrompt(id);
      logger.debug('Unregistered built-in tools system prompt', {
        promptId: id,
      });
    };
  }, [
    buildPrompt,
    registerSystemPrompt,
    unregisterSystemPrompt,
    availableTools.length,
    server,
  ]);

  return null;
}
