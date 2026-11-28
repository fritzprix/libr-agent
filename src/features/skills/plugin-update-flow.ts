import { toast } from 'sonner';
import type { TFunction } from 'i18next';
import {
  installGitHubSkills,
  previewGitHubSkillInstall,
  previewSessionPluginInstall,
} from '@/lib/backend/skills';
import type { SkillImportPreview, SkillImportConflict } from '@/types/skills';
import type { PendingInstall } from './skills-management-types';
import { defaultPluginOptions, notifyImportWarnings } from './skills-management-utils';

/**
 * The shared `#2927` D6 update flow (#2954 gatekeeper G1.6): after the one
 * Agree approves the whole update (changed hooks and MCP servers included),
 * preview the fresh source, keep only genuine user-data conflicts for the
 * review dialog, and otherwise re-download with `approveHooks=true` — the
 * single place that grants hook/MCP execution for an update.
 *
 * Used by both callers so the approval semantics cannot drift apart:
 * - the Skills view (`useSkillsManagementPanel`), global scope, and
 * - `/plugin update` (`useSessionPluginInstall`), session or global scope.
 */

/** Split of the update preview the caller needs to continue the flow. */
export interface PluginUpdatePlan {
  preview: SkillImportPreview;
  /** Skills that belong to the user — the only ones left for the dialog. */
  userConflictNames: string[];
  /** System-bundle collisions — always excluded (never overwritten). */
  systemConflictNames: string[];
}

/**
 * Preview an update and split its conflicts. Returns `null` when the caller
 * should stop (an empty source URL, or the preview failed — the toast is
 * already shown).
 */
export async function planPluginUpdate(
  sourceUrl: string,
  scope: 'global' | 'session',
  sessionId: string | undefined,
  t: TFunction,
): Promise<PluginUpdatePlan | null> {
  const toastId = toast.loading(
    t('settings.skills.githubInspecting', 'Inspecting plugin source...'),
  );
  try {
    const preview =
      scope === 'session' && sessionId
        ? await previewSessionPluginInstall(sessionId, sourceUrl)
        : await previewGitHubSkillInstall(sourceUrl);
    toast.dismiss(toastId);
    // Review-worthy conflicts: in global scope only skills that belong to
    // the user (#2954 G1.2). In session scope, existing skills carry the
    // 'workspace' origin (there is no user directory there), so every
    // non-system collision keeps the old review behavior. System-bundle
    // collisions are always excluded and never reviewed.
    const reviewable = (origin: SkillImportConflict['existingOrigin']) =>
      scope === 'global'
        ? origin === 'user'
        : origin !== 'system';
    return {
      preview,
      userConflictNames: preview.conflicts
        .filter((c) => reviewable(c.existingOrigin))
        .map((c) => c.name),
      systemConflictNames: preview.conflicts
        .filter((c) => c.existingOrigin === 'system')
        .map((c) => c.name),
    };
  } catch (error) {
    toast.dismiss(toastId);
    toast.error(error instanceof Error ? error.message : String(error), {
      id: toastId,
    });
    return null;
  }
}

/**
 * The conflict-dialog state an update opens when user data needs review.
 * The D6 Agree already approved hooks and MCP servers, so `approveHooks` is
 * `true` here — the dialog itself only collects overwrite choices.
 */
export function pendingInstallForUpdate(
  plan: PluginUpdatePlan,
  sourceUrl: string,
): PendingInstall {
  return {
    kind: 'github',
    sourceValue: sourceUrl,
    preview: plan.preview,
    selectedOverwriteNames: plan.userConflictNames,
    approveHooks: true,
    pluginOptions: defaultPluginOptions(plan.preview),
  };
}

/**
 * The re-download itself: overwrite everything, exclude only system
 * collisions, and grant hooks/MCP (`approveHooks=true`, covered by the D6
 * Agree). Returns the import result for the caller's success handling.
 */
export async function installPluginUpdate(
  plan: PluginUpdatePlan,
  sourceUrl: string,
): Promise<import('@/types/skills').SkillImportResult> {
  return installGitHubSkills(
    sourceUrl,
    true,
    plan.systemConflictNames,
    true,
    {},
  );
}

/** Success toast shared by both update flows (#2927 D5: applies on reopen). */
export function toastPluginUpdated(t: TFunction): void {
  toast.success(
    t(
      'settings.skills.pluginUpdatedReload',
      'Plugin updated. It applies when a session opens again; type /reload to apply it now.',
    ),
  );
}

/**
 * #2954 G1.3: after an update succeeds, the D6 badge must clear even though
 * the source id list did not. The badge state lives in the Skills panel
 * hook, but updates also run from `/plugin update` in a session, so the
 * flow signals completion through this event instead of a direct callback.
 */
export const PLUGIN_UPDATED_EVENT = 'libragent:plugin-updated';

export function notifyPluginUpdated(): void {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new Event(PLUGIN_UPDATED_EVENT));
  }
}

export { notifyImportWarnings };
