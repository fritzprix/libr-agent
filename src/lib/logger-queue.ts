import { invoke } from '@tauri-apps/api/core';

import { LogLevel } from './logger-types';

interface LogEntry {
  level: LogLevel;
  message: string;
  timestamp: number;
}

class LogQueue {
  private queue: LogEntry[] = [];
  private readonly batchSize = 50;
  private readonly flushInterval = 500;
  private timer: NodeJS.Timeout | null = null;

  enqueue(level: LogLevel, message: string): void {
    this.queue.push({
      level,
      message,
      timestamp: Date.now(),
    });

    if (this.queue.length >= this.batchSize) {
      void this.flush();
    } else if (!this.timer) {
      this.timer = setTimeout(() => {
        void this.flush();
      }, this.flushInterval);
    }
  }

  async flush(): Promise<void> {
    if (this.queue.length === 0) {
      return;
    }

    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }

    const entries = [...this.queue];
    this.queue = [];

    try {
      await invoke<void>('log_batch', { entries });
    } catch (error) {
      console.error('[Logger] Failed to flush log batch:', error);
      this.queue = [...entries, ...this.queue].slice(-this.batchSize);
    }
  }
}

const globalLogQueue = new LogQueue();

function logToBackend(level: LogLevel, message: string): void {
  globalLogQueue.enqueue(level, message);
}

export const trace = (message: string) => logToBackend(LogLevel.Trace, message);
export const debug = (message: string) => logToBackend(LogLevel.Debug, message);
export const info = (message: string) => logToBackend(LogLevel.Info, message);
export const warn = (message: string) => logToBackend(LogLevel.Warn, message);
export const error = (message: string) => logToBackend(LogLevel.Error, message);
