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

    // Version 2: Add title index to playbooks and reset data for schema change
    this.version(2)
      .stores({
        playbooks: '&id, agentId, createdAt, updatedAt, goal, title',
      })
      .upgrade(async (trans) => {
        // Clear old playbooks as the schema has changed significantly (inputs added)
        await trans.table('playbooks').clear();
      });

    // Populate hook: Seed default assistants only on fresh DB creation
    this.on('populate', async () => {
      const now = new Date();

      const bootstrapAssistant: Assistant = {
        id: createId(),
        name: 'Bootstrap Assistant',
        systemPrompt:
          'You are the Bootstrap Assistant for LibrAgent.\n' +
          'Your job is to help users bootstrap their environment by detecting the platform, checking for installed tools, and guiding them through installation.\n\n' +
          'Workflow:\n' +
          '1) Detect Platform: Use "detect_platform" to identify the OS and shell.\n' +
          '2) Check Tools: Use "check_tool_installed" to get the verification command, then run it with "execute_shell" or "execute_windows_cmd".\n' +
          '3) Guide Installation: If a tool is missing, use "get_bootstrap_guide" to provide installation instructions.\n' +
          '4) Configure MCP: If the user provides MCP server config, add it to the backend registry.\n\n' +
          'Rules:\n' +
          '- ALWAYS detect the platform first.\n' +
          '- Verify tool installation before assuming it exists.\n' +
          '- Use the "bootstrap" tools for guidance and "workspace" tools for execution.\n' +
          '- Be helpful and guide the user step-by-step.',
        mcpServerIds: [],
        deletionProtected: true,
        localServices: [],
        allowedBuiltInServiceAliases: [
          'bootstrap',
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
