/**
 * @file LibrAgent Global Logger System
 *
 * Public facade for the logging subsystem. The implementation is split across
 * focused modules so callers can keep importing from `@/lib/logger` without
 * caring about queueing, config persistence, or log-file management details.
 */

export { Logger, getLogger, log } from './logger-core';
export { logFileManager, type LogFileManager } from './logger-file-manager';
export { debug, error, info, trace, warn } from './logger-queue';
export { logUtils } from './logger-utils';
export { LogLevel, type LoggerConfig } from './logger-types';
