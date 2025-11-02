/**
 * Bootstrap Server Module
 *
 * Built-in MCP server for platform detection and dependency installation guides
 */

export { default } from './server';
export { bootstrapToolsSchema } from './tools';
export { detectPlatform, getCheckCommand } from './platform-detector';
export { BOOTSTRAP_GUIDES } from './guides';
export type { PlatformInfo } from './platform-detector';
export type { InstallationStep, InstallationMethod, ToolGuide } from './guides';
