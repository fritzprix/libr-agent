import { safeInvoke } from './core';
import type {
  CompatibilityStatus,
  ConflictStrategy,
  MigrationSectionReport,
  SectionPreview,
} from './migration';

export interface OrgBundleExportInfo {
  file_path: string;
  file_size_bytes: number;
  org_id: string;
  org_name: string;
  sections: string[];
  assistant_ids: string[];
  mcp_ids: string[];
}

export interface OrgBundleImportResult {
  org_id: string;
  org_name: string;
  teamwork_import_path: string;
  /** Staging path for org-root workspace skills (`.libragent/skills/`). */
  workspace_skills_import_path?: string | null;
  sections_imported: Record<string, MigrationSectionReport>;
  total_imported: number;
  total_skipped: number;
  total_errors: number;
}

export interface OrgBundlePreview {
  format_version: number;
  app_version: string | null;
  exported_at: string | null;
  compatibility: CompatibilityStatus;
  org_id: string | null;
  org_name: string | null;
  source_root_session_id: string | null;
  assistant_ids: string[];
  mcp_ids: string[];
  sections: SectionPreview[];
  total_size_bytes: number;
  file_path: string;
}

export async function exportOrgBundle(
  orgId: string,
  outputPath: string,
  includeSensitiveData: boolean,
  password?: string,
): Promise<OrgBundleExportInfo> {
  return safeInvoke<OrgBundleExportInfo>('export_org_bundle', {
    orgId,
    outputPath,
    includeSensitiveData,
    password: password || null,
  });
}

export async function importOrgBundle(
  filePath: string,
  conflictStrategy: ConflictStrategy,
  password?: string,
): Promise<OrgBundleImportResult> {
  return safeInvoke<OrgBundleImportResult>('import_org_bundle', {
    filePath,
    conflictStrategy,
    password: password || null,
  });
}

export async function inspectOrgBundle(
  filePath: string,
  password?: string,
): Promise<OrgBundlePreview> {
  return safeInvoke<OrgBundlePreview>('inspect_org_bundle', {
    filePath,
    password: password || null,
  });
}
