import { safeInvoke } from '@/lib/backend';
import { getLogger } from '@/lib/logger';

const logger = getLogger('CompactContextService');

export interface CompactContextRecord {
  id: string;
  sessionId: string;
  fromId: string;
  toId: string;
  summary: string;
  createdAt: number;
}

export class CompactContextService {
  /**
   * Get compacted context for a session
   */
  async getCompactContext(
    sessionId: string,
  ): Promise<CompactContextRecord | null> {
    try {
      return await safeInvoke<CompactContextRecord | null>(
        'agent_get_compact_context',
        {
          sessionId,
        },
      );
    } catch (error) {
      logger.error(
        `Failed to get compact context for session ${sessionId}:`,
        error,
      );
      return null;
    }
  }

  /**
   * Save compacted context for a session
   */
  async saveCompactContext(
    sessionId: string,
    record: CompactContextRecord,
  ): Promise<boolean> {
    try {
      const response = await safeInvoke<{ success: boolean }>(
        'agent_save_compact_context',
        {
          sessionId,
          record,
        },
      );
      return response.success;
    } catch (error) {
      logger.error(
        `Failed to save compact context for session ${sessionId}:`,
        error,
      );
      return false;
    }
  }
}

export const compactContextService = new CompactContextService();
