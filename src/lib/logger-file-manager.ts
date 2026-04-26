import { invoke } from '@tauri-apps/api/core';

export interface LogFileManager {
  getLogDirectory(): Promise<string>;
  backupCurrentLog(): Promise<string>;
  clearCurrentLog(): Promise<void>;
  listLogFiles(): Promise<string[]>;
}

class TauriLogFileManager implements LogFileManager {
  async getLogDirectory(): Promise<string> {
    return await invoke<string>('get_app_logs_dir');
  }

  async backupCurrentLog(): Promise<string> {
    return await invoke<string>('backup_current_log');
  }

  async clearCurrentLog(): Promise<void> {
    await invoke<void>('clear_current_log');
  }

  async listLogFiles(): Promise<string[]> {
    return await invoke<string[]>('list_log_files');
  }
}

export const logFileManager = new TauriLogFileManager();
