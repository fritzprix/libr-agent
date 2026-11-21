import type { TFunction } from 'i18next';
import { toast } from 'sonner';
import type { SkillImportPreview, SkillImportResult } from '@/types/skills';
import type {
  SkillsStatusMessage,
  SkillsVerificationStatus,
} from './skills-management-types';

/** Initial answers for a plugin's `userConfig` options (#2926): the manifest
 *  defaults, so the install dialog opens with sensible values. */
export function defaultPluginOptions(
  preview: SkillImportPreview,
): Record<string, string> {
  const options: Record<string, string> = {};
  for (const entry of preview.userConfig ?? []) {
    options[entry.key] = entry.default ?? '';
  }
  return options;
}

/** Required options the user has not answered yet (#2926). While this is not
 *  empty, the install dialog must not confirm: the backend rejects the install
 *  without them. */
export function missingRequiredPluginOptions(
  preview: SkillImportPreview,
  values: Record<string, string>,
): string[] {
  return (preview.userConfig ?? [])
    .filter((entry) => entry.required)
    .filter((entry) => !(values[entry.key] ?? entry.default ?? '').trim())
    .map((entry) => entry.title || entry.key);
}

export function shouldPromptSkillInstall(
  preview: SkillImportPreview,
  userConflictCount: number,
): boolean {
  return (
    preview.kind === 'plugin' ||
    preview.hooks.length > 0 ||
    // #2924: a plugin that declares MCP servers needs the install dialog so
    // the user can approve starting them.
    preview.mcpRequireApproval === true ||
    userConflictCount > 0
  );
}

export function formatImportSuccess(
  t: TFunction,
  importedCount: number,
  overwrittenCount: number,
  skippedCount: number,
): string {
  if (importedCount === 0 && skippedCount > 0) {
    return t('settings.skills.importSkippedOnly', {
      count: skippedCount,
      defaultValue_one: 'Skipped {{count}} conflicting skill',
      defaultValue_other: 'Skipped {{count}} conflicting skills',
    });
  }

  if (overwrittenCount > 0 && skippedCount > 0) {
    return t('settings.skills.importSuccessWithOverwriteAndSkipped', {
      count: importedCount,
      overwrittenCount,
      skippedCount,
      defaultValue_one:
        'Imported {{count}} skill ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
      defaultValue_other:
        'Imported {{count}} skills ({{overwrittenCount}} overwritten, {{skippedCount}} skipped)',
    });
  }

  if (overwrittenCount > 0) {
    return t('settings.skills.importSuccessWithOverwrite', {
      count: importedCount,
      overwrittenCount,
      defaultValue_one:
        'Imported {{count}} skill ({{overwrittenCount}} overwritten)',
      defaultValue_other:
        'Imported {{count}} skills ({{overwrittenCount}} overwritten)',
    });
  }

  if (skippedCount > 0) {
    return t('settings.skills.importSuccessWithSkipped', {
      count: importedCount,
      skippedCount,
      defaultValue_one: 'Imported {{count}} skill ({{skippedCount}} skipped)',
      defaultValue_other:
        'Imported {{count}} skills ({{skippedCount}} skipped)',
    });
  }

  return t('settings.skills.importSuccess', {
    count: importedCount,
    defaultValue_one: 'Imported {{count}} skill',
    defaultValue_other: 'Imported {{count}} skills',
  });
}

/// Surface non-fatal install warnings (#2924 gatekeeper G1.2): the install
/// itself succeeded, but items the user approved (e.g. plugin MCP servers)
/// may not be running — the user must see why.
export function notifyImportWarnings(result: SkillImportResult): void {
  for (const warning of result.warnings ?? []) {
    toast.warning(warning, { duration: 10_000 });
  }
}

export function getSkillsStatusMessage(
  verificationStatus: SkillsVerificationStatus,
  errorMessage: string,
  skillsCount: number,
  t: TFunction,
): SkillsStatusMessage {
  if (verificationStatus === 'loading') {
    return {
      kind: 'loading',
      text: t('settings.general.verifying', 'Verifying...'),
      tone: 'text-muted-foreground',
    };
  }

  if (verificationStatus === 'error') {
    return {
      kind: 'error',
      text:
        errorMessage ||
        t('settings.general.invalidDirectory', 'Invalid directory'),
      tone: 'text-destructive',
    };
  }

  return {
    kind: 'success',
    text: t('settings.skills.installedCount', {
      count: skillsCount,
      defaultValue: '{{count}} installed skills available',
    }),
    tone: 'text-success',
  };
}
