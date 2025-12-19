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

    // Populate hook removed to prevent race conditions with AssistantContext
  }

  /**
   * Ensures that the default assistants exist in the database.
   * Checks for existence by name. If not found, creates them.
   */
  public async ensureDefaultAssistants(): Promise<void> {
    await this.transaction('rw', this.assistants, async () => {
      const now = new Date();

      const currentAssistants = await this.assistants.toArray();
      const existingNames = new Set(currentAssistants.map((a) => a.name));

      const assistantsToAdd: Assistant[] = [];

      if (!existingNames.has('Bootstrap Assistant')) {
        assistantsToAdd.push({
          id: createId(),
          name: 'Bootstrap Assistant',
          systemPrompt:
            'You are the Bootstrap Assistant for LibrAgent.\n' +
            'Your job is to help users bootstrap their environment by detecting the platform, checking for installed tools, and guiding them through installation.\n\n' +
            'Strategy:\n' +
            '- Goal & Plan: Always start by setting a goal and plan.\n' +
            '- Detect Platform: Always identify the OS and shell environment first.\n' +
            '- Verify Dependencies: Check if necessary tools are installed before attempting to use them.\n' +
            '- Guide Installation: If a tool is missing, provide clear, step-by-step installation instructions.\n' +
            '- Configure Integration: Assist the user in configuring and connecting external tools or servers (MCP).',
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
        });
      }

      if (!existingNames.has('Libr Assistant')) {
        assistantsToAdd.push({
          id: createId(),
          name: 'Libr Assistant',
          systemPrompt:
            'You are the Libr Assistant: a general-purpose knowledge and automation agent.\n\n' +
            'Strategy:\n' +
            "- Analyze Intent: Upon receiving a request, deeply analyze the user's intent. Ask clarifying questions only if absolutely necessary.\n" +
            '- Plan & Execute: Always start by setting a goal and plan, then execute them systematically.\n' +
            '- Record Memories: Since memory is limited, periodically record your thoughts and important information.\n' +
            '- Think Deeper: If a problem becomes difficult, always take a step back and think deeper to find a solution.',
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
        });
      }

      if (assistantsToAdd.length > 0) {
        await this.assistants.bulkAdd(assistantsToAdd);
      }
    });
  }
}
