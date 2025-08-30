/**
 * Worker-Safe Logger for Content Store
 *
 * Provides logging functionality that works in both main thread and worker environments
 */

// Worker-safe logger that falls back to console if Tauri logger is not available
export const createWorkerSafeLogger = (context: string) => {
  // Fallback to console logger for Worker environment
  return {
    debug: (message: string, data?: unknown) => {
      console.log(`[${context}][DEBUG] ${message}`, data || '');
    },
    info: (message: string, data?: unknown) => {
      console.log(`[${context}][INFO] ${message}`, data || '');
    },
    warn: (message: string, data?: unknown) => {
      console.warn(`[${context}][WARN] ${message}`, data || '');
    },
    error: (message: string, data?: unknown) => {
      console.error(`[${context}][ERROR] ${message}`, data || '');
    },
  };
};

export const logger = createWorkerSafeLogger('content-store');
