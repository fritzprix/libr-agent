import { invoke } from '@tauri-apps/api/core';

export interface BuiltinServerMetadata {
  displayName: string;
  description: string;
  icon?: string;
}

export interface BuiltinServerInfo {
  name: string;
  metadata: BuiltinServerMetadata;
  toolCount: number;
}

/**
 * Lists all available built-in MCP server definitions.
 * unique from 'active' servers, this returns the static configuration of what *could* be enabled.
 */
export async function listAvailableBuiltinServerDefinitions(): Promise<
  BuiltinServerInfo[]
> {
  return invoke<BuiltinServerInfo[]>(
    'list_available_builtin_server_definitions',
  );
}
