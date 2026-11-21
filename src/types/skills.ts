/**
 * Represents metadata for a skill.
 * This should match the Rust struct in `src-tauri/src/commands/skill_commands.rs`.
 */
export interface SkillMetadata {
  name: string;
  description: string;
  path: string;
  /**
   * Preferred alias path for workspace MCP reads when the skill lives under a
   * managed/session-known root. Falls back to `path` when no alias exists.
   */
  aliasPath?: string;
  /**
   * Source of the skill.
   * - 'global': Located in the global skills directory.
   * - 'assistant': Located in the assistant's specific skills directory.
   * - 'workspace': Located in the workspace's local skills directory.
   * - undefined: Source is unknown or not relevant (e.g. raw directory scan).
   */
  source?: 'global' | 'assistant' | 'workspace';
  /**
   * Physical ownership of the skill content.
   * - 'system': Bundled read-only skill shipped with the app.
   * - 'user': Managed user-global skill installed into app data.
   * - 'assistant': Assistant-scoped override stored in app data.
   * - 'workspace': Workspace-local skill.
   */
  origin?: 'system' | 'user' | 'assistant' | 'workspace';
}

export interface PluginSourceOverview {
  id: string;
  sourceUrl: string;
  skillNames: string[];
  hookEvents: string[];
  hooksApproved: boolean;
  /** Manifest `name` from `.claude-plugin/plugin.json`, when present. */
  pluginName?: string;
  pluginVersion?: string;
  pluginDescription?: string;
  /** Whether the plugin is enabled (#2927). */
  enabled: boolean;
  /** Source commit recorded at install, when GitHub exposed one (#2927). */
  installedCommit?: string;
}

export interface ManagedSkillsOverview {
  systemDirectory: string;
  userDirectory: string;
  systemSkills: SkillMetadata[];
  userSkills: SkillMetadata[];
  effectiveSkills: SkillMetadata[];
  pluginSources: PluginSourceOverview[];
}

export interface SkillImportCandidate {
  name: string;
  description: string;
}

export interface SkillImportConflict {
  name: string;
  existingOrigin: 'system' | 'user' | 'assistant' | 'workspace' | 'unknown';
  existingPath: string;
}

export type SkillImportKind = 'skillBundle' | 'plugin';

export type HookPreviewExecutionStatus = 'executed' | 'stubbed';

export interface HookPreviewEntry {
  event: string;
  matcher?: string;
  command: string;
  powershell?: string;
  scriptBody?: string;
  executionStatus: HookPreviewExecutionStatus;
}

/** `.claude-plugin/plugin.json` summary shown in the install preview (#2921). */
export interface PluginManifestSummary {
  name: string;
  version?: string;
  description?: string;
}

/** One plugin-declared MCP server in the install preview (#2924). */
export interface McpServerPreviewEntry {
  name: string;
  transport: 'stdio' | 'http';
  command?: string;
  /** Command arguments joined with spaces, as declared (#2924 D3). */
  args?: string;
  /** Environment variable NAMES the server sets (values never shown). */
  envNames?: string[];
  url?: string;
  /** HTTP header NAMES the server sends (values never shown). */
  headerNames?: string[];
}

export interface SkillImportPreview {
  kind: SkillImportKind;
  discoveredSkills: SkillImportCandidate[];
  conflicts: SkillImportConflict[];
  hooks: HookPreviewEntry[];
  hooksRequireApproval: boolean;
  /** MCP servers the plugin declares (#2924). */
  mcpServers?: McpServerPreviewEntry[];
  /** Starting the plugin's MCP servers needs install-time approval (#2924). */
  mcpRequireApproval?: boolean;
  /** Slash command names the plugin's `commands/` provides (#2922). */
  pluginCommands?: string[];
  /** Subagent definitions the plugin's `agents/` provides (#2923). */
  pluginAgents?: PluginAgentPreview[];
  /** Present when the source carried a `.claude-plugin/plugin.json`. */
  pluginManifest?: PluginManifestSummary;
  /** `userConfig` options the plugin asks for (#2926). */
  userConfig?: UserConfigPreviewEntry[];
}

/** One `userConfig` option asked for at install (#2926). Mirrors Rust
 *  `UserConfigPreviewEntry`. */
export interface UserConfigPreviewEntry {
  key: string;
  title: string;
  description?: string;
  /** 'string' | 'number' | 'boolean' | 'directory' | 'file' */
  valueType: string;
  /** Masked input; stored in secure storage, never in a plain file. */
  sensitive: boolean;
  required: boolean;
  default?: string;
}

/** One `agents/*.md` subagent definition shown in the install preview
 *  (#2923). `unmappedTools` are Claude Code tools with no LibrAgent
 *  mapping: they are shown here and never silently granted.
 *  `grantedServices` are the LibrAgent builtin services the declared
 *  `tools` map to (gatekeeper G1.2: the user must see what IS granted;
 *  `workspace` includes file writes and shell). `grantsAllServices` is
 *  true when the definition has no `tools` line at all. */
export interface PluginAgentPreview {
  name: string;
  description: string;
  unmappedTools: string[];
  grantedServices: string[];
  grantsAllServices: boolean;
  model?: string;
}

export interface SkillImportResult {
  importedNames: string[];
  overwrittenNames: string[];
  skippedNames: string[];
  /** Non-fatal problems the user must see, e.g. an approved plugin MCP
   *  server that could not be installed (#2924 gatekeeper G1.2). */
  warnings?: string[];
}

/** Where an active plugin was installed. Mirrors Rust `PluginScope`. */
export type PluginScope = 'global' | 'session';

/** Mirrors Rust `SessionPluginEntry` (camelCase). */
export interface SessionPluginEntry {
  id: string;
  sourceUrl: string;
  scope: PluginScope;
  /**
   * Unix **seconds** of the install (the backend `i64`). Multiply by 1000
   * before building a `Date` — it is not an ISO string (#2927).
   */
  installedAt: number;
  skillNames: string[];
  hookEvents: string[];
  hooksApproved: boolean;
  /** Whether the plugin is enabled (#2927). */
  enabled: boolean;
  /** Manifest `version` from `.claude-plugin/plugin.json`, when present. */
  version?: string;
  /** Source commit recorded at install, when it is known (#2927). */
  commit?: string;
}

export type SessionPluginAction = 'install' | 'uninstall';

/** Mirrors Rust `SessionPluginHistoryEntry` (camelCase). */
export interface SessionPluginHistoryEntry {
  sourceId: string;
  sourceUrl: string;
  action: SessionPluginAction;
  /** Unix **seconds** of the action (the backend `i64`), not an ISO string. */
  at: number;
}

/** Mirrors Rust `SessionPluginsOverview` (camelCase). */
export interface SessionPluginsOverview {
  active: SessionPluginEntry[];
  history: SessionPluginHistoryEntry[];
}
