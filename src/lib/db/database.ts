import type {
  Assistant,
  Message,
  Session,
  MCPServerEntity,
} from '@/models/chat';
import Dexie, { Table } from 'dexie';
import type { DatabaseObject } from './types';
import type { Playbook } from '@/types/playbook';
import { createId } from '@paralleldrive/cuid2';

/**
 * A singleton class that extends Dexie to provide a local database service.
 * It defines the database schema, handles versioning, and provides access
 * to the database tables.
 */
export class LocalDatabase extends Dexie {
  private static instance: LocalDatabase;

  /**
   * Gets the singleton instance of the LocalDatabase.
   * @returns The singleton LocalDatabase instance.
   */
  public static getInstance(): LocalDatabase {
    if (!LocalDatabase.instance) {
      LocalDatabase.instance = new LocalDatabase();
    }
    return LocalDatabase.instance;
  }

  /**
   * Resets the singleton instance. Used for testing purposes.
   * WARNING: This should only be called in test environments.
   */
  public static resetInstance(): void {
    if (LocalDatabase.instance) {
      LocalDatabase.instance.close();
      LocalDatabase.instance = null as unknown as LocalDatabase;
    }
  }

  assistants!: Table<Assistant, string>;
  mcpServers!: Table<MCPServerEntity, string>;
  objects!: Table<DatabaseObject<unknown>, string>;
  sessions!: Table<Session, string>;
  messages!: Table<Message, string>;
  // Groups are now handled in-memory (no persistent IndexedDB table)
  playbooks!: Table<
    Playbook & { id: string; createdAt?: Date; updatedAt?: Date },
    string
  >;
  // File content is persisted by the Rust backend; frontend no longer
  // maintains fileStores/fileContents/fileChunks in IndexedDB.

  constructor() {
    super('MCPAgentDB');

    // Version 1: Initial consolidated schema
    // Historical versions (1-9) have been squashed into v1 for fresh installations
    this.version(1).stores({
      assistants: '&id, createdAt, updatedAt, name',
      mcpServers: '&id, name, createdAt, updatedAt, isActive',
      objects: '&key, createdAt, updatedAt',
      sessions: '&id, createdAt, updatedAt',
      messages: '&id, sessionId, [sessionId+threadId], createdAt',
      playbooks: '&id, agentId, createdAt, updatedAt, goal',
    });

    // Populate hook: Seed default assistants only on fresh DB creation
    this.on('populate', async () => {
      const now = new Date();

      const bootstrapAssistant: Assistant = {
        id: createId(),
        name: 'Bootstrap Assistant',
        systemPrompt:
          'You are the Bootstrap Assistant for LibrAgent.\n' +
          'Your job is to configure MCP servers and install required dependencies based on user requests.\n\n' +
          'Workflow:\n' +
          '1) When user provides MCP server configuration (command, args, env), add it to the backend MCP registry.\n' +
          '2) Check if required dependencies exist (e.g., Node.js, npx, Python packages, or specific commands).\n' +
          '3) If dependencies are missing, guide the user through installation:\n' +
          '   - Use builtin_workspace__execute_shell (Unix) or builtin_workspace__execute_windows_cmd (Windows)\n' +
          '   - Verify installation with version checks or test commands\n' +
          '4) Test MCP server connectivity after installation using builtin_mcp_manager__get_server_info.\n' +
          '5) Use planning tools to track installation steps (create_goal, add_todo, mark_todo).\n\n' +
          'Rules:\n' +
          '- Always ask for confirmation before executing system commands or installing packages.\n' +
          "- Detect the user's platform (check environment or ask) to provide correct commands.\n" +
          '- Provide clear error messages and troubleshooting steps if installation fails.\n' +
          '- After successful setup, summarize what was installed and how to verify it.',
        mcpServerIds: [],
        deletionProtected: true,
        localServices: [],
        allowedBuiltInServiceAliases: [
          'mcp_manager',
          'workspace',
          'planning',
          'assistant_manager',
        ],
        createdAt: now,
        updatedAt: now,
      };

      const librAssistant: Assistant = {
        id: createId(),
        name: 'Libr Assistant',
        systemPrompt:
          'You are the Libr Assistant: a general-purpose knowledge and automation agent.\n' +
          'Workflow:\n' +
          '1) Analyze the request and set/maintain goals via planning tools (create_goal, add_todo, get_current_state).\n' +
          '2) Prefer local knowledge first: query builtin_content_store__keywordSimilaritySearch and read with readContent.\n' +
          '3) If missing, use builtin_workspace__read_file to search the workspace; with permission, use browser tools to gather web content, then add to content_store via addContent.\n' +
          '4) Persist results to the workspace, cite sources (paths/URIs/URLs), and update planning state. Keep answers concise.\n' +
          'Rules:\n' +
          '- Ask before web browsing or executing commands.\n' +
          '- Use minimal changes; confirm write paths.\n' +
          '- Provide sources and next-step suggestions.',
        mcpServerIds: [],
        deletionProtected: true,
        localServices: [],
        allowedBuiltInServiceAliases: [
          'contentstore',
          'workspace',
          'browser',
          'planning',
          'playbook',
          'mcp_manager',
          'ui',
          'assistant_manager',
        ],
        createdAt: now,
        updatedAt: now,
      };

      await this.assistants.bulkAdd([bootstrapAssistant, librAssistant]);
    });
  }
}
