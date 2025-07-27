import {
  debug,
  info,
  warn,
  error as logError,
  trace,
} from "@tauri-apps/plugin-log";

export class Logger {
  private static defaultContext = "TauriAgent";

  private static formatLogMessage(
    message: string,
    args: unknown[],
    defaultContext: string,
  ): { formattedMessage: string; context: string } {
    let actualContext = defaultContext;
    let logMessage = message;
    let logArgs = [...args];

    // Check if the last argument is a context string
    if (logArgs.length > 0 && typeof logArgs[logArgs.length - 1] === "string") {
      actualContext = logArgs.pop() as string; // Remove and use as context
    }

    // Format the message and remaining arguments
    if (logArgs.length > 0) {
      const formattedArgs = logArgs.map((arg) => {
        if (typeof arg === "object" && arg !== null) {
          return JSON.stringify(arg);
        }
        return String(arg);
      });
      logMessage = `${logMessage} ${formattedArgs.join(" ")}`;
    }
    return { formattedMessage: logMessage, context: actualContext };
  }

  static async debug(message: string, ...args: unknown[]): Promise<void> {
    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await debug(`[${context}] ${formattedMessage}`);
  }

  static async info(message: string, ...args: unknown[]): Promise<void> {
    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await info(`[${context}] ${formattedMessage}`);
  }

  static async warn(message: string, ...args: unknown[]): Promise<void> {
    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await warn(`[${context}] ${formattedMessage}`);
  }

  static async error(message: string, ...args: unknown[]): Promise<void> {
    let errorObj: Error | undefined;
    let remainingArgs = [...args];

    // Check if the last argument is an Error object
    if (
      remainingArgs.length > 0 &&
      remainingArgs[remainingArgs.length - 1] instanceof Error
    ) {
      const popped = remainingArgs.pop();
      if (popped instanceof Error) {
        errorObj = popped;
      }
    }

    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      remainingArgs,
      Logger.defaultContext,
    );
    const errorMsg = errorObj
      ? `${formattedMessage}: ${errorObj.message}`
      : formattedMessage;
    await logError(`[${context}] ${errorMsg}`);
  }

  static async trace(message: string, ...args: unknown[]): Promise<void> {
    const { formattedMessage, context } = Logger.formatLogMessage(
      message,
      args,
      Logger.defaultContext,
    );
    await trace(`[${context}] ${formattedMessage}`);
  }
}

// Convenience functions for common logging patterns (global logger)
export const log = {
  debug: (message: string, ...args: unknown[]) => Logger.debug(message, ...args),
  info: (message: string, ...args: unknown[]) => Logger.info(message, ...args),
  warn: (message: string, ...args: unknown[]) => Logger.warn(message, ...args),
  error: (message: string, ...args: unknown[]) => Logger.error(message, ...args),
  trace: (message: string, ...args: unknown[]) => Logger.trace(message, ...args),
};

// Function to get a context-specific logger instance
export function getLogger(contextName: string) {
  return {
    debug: (message: string, ...args: unknown[]) =>
      Logger.debug(message, ...args, contextName),
    info: (message: string, ...args: unknown[]) =>
      Logger.info(message, ...args, contextName),
    warn: (message: string, ...args: unknown[]) =>
      Logger.warn(message, ...args, contextName),
    error: (message: string, ...args: unknown[]) =>
      Logger.error(message, ...args, contextName),
    trace: (message: string, ...args: unknown[]) =>
      Logger.trace(message, ...args, contextName),
  };
}
