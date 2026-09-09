import { safeInvoke } from './core';

export type SessionExportFormat = 'markdown' | 'atif';

export interface ExportSessionFileArgs {
  sessionId: string;
  format: SessionExportFormat;
}

/**
 * Saves a session's persisted history via the native Save File dialog.
 * Markdown and ATIF-v1.7 are generated in Rust from the SQLite snapshot.
 */
export async function exportSessionFile(
  args: ExportSessionFileArgs,
): Promise<string> {
  return safeInvoke<string>('export_session_file', { ...args });
}
