export enum LogLevel {
  Trace = 'trace',
  Debug = 'debug',
  Info = 'info',
  Warn = 'warn',
  Error = 'error',
}

export interface LoggerConfig {
  enableFileLogging: boolean;
  autoBackupOnStartup: boolean;
  maxBackupFiles: number;
  logLevel: 'trace' | 'debug' | 'info' | 'warn' | 'error';
  maxMessageLength: number;
}
