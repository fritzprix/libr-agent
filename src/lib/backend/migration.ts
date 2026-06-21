import { safeInvoke } from './core';

export type ConflictStrategy = 'skip' | 'overwrite' | 'merge';

export interface MigrationExportInfo {
  file_path: string;
  file_size_bytes: number;
  sections: string[];
}

export interface MigrationSectionReport {
  success: number;
  skipped: number;
  errors: string[];
}

export interface MigrationImportResult {
  sections_imported: Record<string, MigrationSectionReport>;
  total_imported: number;
  total_skipped: number;
  total_errors: number;
}

export interface SectionPreview {
  name: string;
  item_count: number;
  size_bytes: number;
}

export type CompatibilityStatus =
  | 'Compatible'
  | { NewerVersion: { message: string } }
  | { Incompatible: { message: string } };

export interface MigrationPreview {
  format_version: number;
  app_version: string | null;
  exported_at: string | null;
  compatibility: CompatibilityStatus;
  sections: SectionPreview[];
  total_size_bytes: number;
  file_path: string;
}

export async function exportMigration(
  outputDir: string,
  includeSensitiveData: boolean,
  password?: string,
): Promise<MigrationExportInfo> {
  return safeInvoke<MigrationExportInfo>('export_migration', {
    outputPath: outputDir,
    includeSensitiveData: includeSensitiveData,
    password: password || null,
  });
}

export async function importMigration(
  filePath: string,
  conflictStrategy: ConflictStrategy,
  password?: string,
): Promise<MigrationImportResult> {
  return safeInvoke<MigrationImportResult>('import_migration', {
    filePath: filePath,
    conflictStrategy: conflictStrategy,
    password: password || null,
  });
}

export async function inspectMigration(
  filePath: string,
  password?: string,
): Promise<MigrationPreview> {
  return safeInvoke<MigrationPreview>('inspect_migration', {
    filePath: filePath,
    password: password || null,
  });
}

// Post-import: MCP 서버 재인증
export async function reverifyMcpServers(): Promise<
  Record<string, 'success' | 'error' | 'skipped'>
> {
  return safeInvoke<Record<string, 'success' | 'error' | 'skipped'>>(
    'reverify_mcp_servers',
    {},
  );
}
