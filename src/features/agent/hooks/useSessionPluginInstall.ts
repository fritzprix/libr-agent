import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useSWRConfig } from 'swr';
import { getLogger } from '@/lib/logger';
import {
  installGitHubSkills,
  installSessionPlugin,
  previewGitHubSkillInstall,
  previewSessionPluginInstall,
  resolvePluginTarget,
} from '@/lib/backend/skills';
import type { PendingInstall } from '@/features/skills/skills-management-types';
import type { PluginScope } from '@/types/skills';
import {
  defaultPluginOptions,
  formatImportSuccess,
  notifyImportWarnings,
  shouldPromptSkillInstall,
} from '@/features/skills/skills-management-utils';
import type { SkillImportConflict } from '@/types/skills';
import { isScopedSkillsKey } from './useScopedSkills';
import { isSessionPluginsKey } from './useSessionPlugins';

const logger = getLogger('useSessionPluginInstall');

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * `/plugin install <git-url>` flow for one session.
 *
 * Preview -> (approval dialog when hooks/conflicts exist) -> install. The
 * dialog state mirrors Extensions → Skills so both share `SkillsConflictDialog`.
 */
export function useSessionPluginInstall(sessionId: string | undefined) {
  const { t } = useTranslation('common');
  const { mutate } = useSWRConfig();
  const [pendingInstall, setPendingInstall] = useState<PendingInstall | null>(
    null,
  );
  const [isInstalling, setIsInstalling] = useState(false);

  const systemConflicts = useMemo<SkillImportConflict[]>(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'system',
      ) ?? [],
    [pendingInstall],
  );
  const overwritableConflicts = useMemo<SkillImportConflict[]>(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin !== 'system',
      ) ?? [],
    [pendingInstall],
  );

  const refreshAfterChange = useCallback(async () => {
    await Promise.all([mutate(isScopedSkillsKey), mutate(isSessionPluginsKey)]);
  }, [mutate]);

  const runInstall = useCallback(
    async (
      repoUrl: string,
      overwriteNames: string[],
      approveHooks: boolean,
      overwritable: SkillImportConflict[],
      system: SkillImportConflict[],
      pluginOptions: Record<string, string> = {},
    ) => {
      if (!sessionId) {
        return;
      }
      const excluded = [
        ...system.map((conflict) => conflict.name),
        ...overwritable
          .map((conflict) => conflict.name)
          .filter((name) => !overwriteNames.includes(name)),
      ];
      const result = await installSessionPlugin(
        sessionId,
        repoUrl,
        overwriteNames.length > 0,
        excluded,
        approveHooks,
        pluginOptions,
      );
      toast.success(
        formatImportSuccess(
          t,
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      await refreshAfterChange();
    },
    [refreshAfterChange, sessionId, t],
  );

  /** Resolves true when the command was consumed (installed or dialog opened). */
  const startInstall = useCallback(
    async (repoUrl: string): Promise<boolean> => {
      if (!sessionId || isInstalling) {
        return false;
      }
      setIsInstalling(true);
      const toastId = toast.loading(
        t(
          'settings.skills.githubInspecting',
          'Inspecting GitHub repository...',
        ),
      );
      try {
        const preview = await previewSessionPluginInstall(sessionId, repoUrl);
        toast.dismiss(toastId);
        const overwritable = preview.conflicts.filter(
          (conflict) => conflict.existingOrigin !== 'system',
        );
        if (shouldPromptSkillInstall(preview, overwritable.length)) {
          setPendingInstall({
            kind: 'github',
            sourceValue: repoUrl,
            preview,
            selectedOverwriteNames: overwritable.map((c) => c.name),
            approveHooks: false,
            pluginOptions: defaultPluginOptions(preview),
          });
          return true;
        }
        await runInstall(
          repoUrl,
          [],
          false,
          [],
          preview.conflicts.filter((c) => c.existingOrigin === 'system'),
        );
        return true;
      } catch (error) {
        logger.error('Failed to install session plugin', error);
        toast.error(errorMessage(error), { id: toastId });
        return false;
      } finally {
        setIsInstalling(false);
      }
    },
    [isInstalling, runInstall, sessionId, t],
  );

  const confirm = useCallback(
    async (approveHooks: boolean) => {
      if (!pendingInstall) {
        return;
      }
      setIsInstalling(true);
      try {
        await runInstall(
          pendingInstall.sourceValue,
          pendingInstall.selectedOverwriteNames,
          approveHooks,
          overwritableConflicts,
          systemConflicts,
          pendingInstall.pluginOptions,
        );
        setPendingInstall(null);
      } catch (error) {
        logger.error('Failed to confirm session plugin install', error);
        toast.error(errorMessage(error));
      } finally {
        setIsInstalling(false);
      }
    },
    [overwritableConflicts, pendingInstall, runInstall, systemConflicts],
  );

  const toggleOverwrite = useCallback((skillName: string) => {
    setPendingInstall((current) =>
      current
        ? {
            ...current,
            selectedOverwriteNames: current.selectedOverwriteNames.includes(
              skillName,
            )
              ? current.selectedOverwriteNames.filter(
                  (name) => name !== skillName,
                )
              : [...current.selectedOverwriteNames, skillName],
          }
        : current,
    );
  }, []);

  const setApproveHooks = useCallback((approveHooks: boolean) => {
    setPendingInstall((current) =>
      current ? { ...current, approveHooks } : current,
    );
  }, []);

  /** Record one `userConfig` answer (#2926). */
  const setPluginOption = useCallback((key: string, value: string) => {
    setPendingInstall((current) =>
      current
        ? {
            ...current,
            pluginOptions: { ...current.pluginOptions, [key]: value },
          }
        : current,
    );
  }, []);

  const handleOpenChange = useCallback((open: boolean) => {
    if (!open) {
      setPendingInstall(null);
    }
  }, []);

  // --- `/plugin update` (#2927 item 4, D6) -------------------------------
  // Resolve the target -> D6 notice -> ONE Agree -> re-download with the
  // install-time approvals already given. User-data conflicts still get the
  // review dialog; hooks/MCP are covered by the Agree.
  const [pendingUpdate, setPendingUpdate] = useState<{
    sourceUrl: string;
    scope: PluginScope;
  } | null>(null);

  /** Resolves true when the command was consumed (notice opened or error). */
  const startUpdate = useCallback(
    async (target: string): Promise<boolean> => {
      if (!sessionId || isInstalling) {
        return false;
      }
      try {
        const entry = await resolvePluginTarget(sessionId, target);
        setPendingUpdate({ sourceUrl: entry.sourceUrl, scope: entry.scope });
        return true;
      } catch (error) {
        logger.error('Failed to resolve plugin update target', error);
        toast.error(errorMessage(error));
        return false;
      }
    },
    [isInstalling, sessionId],
  );

  const runUpdateAfterAgree = useCallback(async () => {
    if (!sessionId || !pendingUpdate || isInstalling) {
      return;
    }
    const { sourceUrl, scope } = pendingUpdate;
    setIsInstalling(true);
    const toastId = toast.loading(
      t('settings.skills.githubInspecting', 'Inspecting GitHub repository...'),
    );
    try {
      const preview =
        scope === 'session'
          ? await previewSessionPluginInstall(sessionId, sourceUrl)
          : await previewGitHubSkillInstall(sourceUrl);
      toast.dismiss(toastId);
      const system = preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'system',
      );
      const overwritable = preview.conflicts.filter(
        (conflict) => conflict.existingOrigin !== 'system',
      );
      setPendingUpdate(null);
      if (overwritable.length > 0) {
        // The D6 Agree already covers hooks and MCP servers; this dialog is
        // only for skills that belong to the user, not to the plugin.
        setPendingInstall({
          kind: 'github',
          sourceValue: sourceUrl,
          preview,
          selectedOverwriteNames: overwritable.map((c) => c.name),
          approveHooks: true,
          pluginOptions: defaultPluginOptions(preview),
        });
        return;
      }
      if (scope === 'session') {
        await runInstall(sourceUrl, [], true, [], system);
      } else {
        const result = await installGitHubSkills(
          sourceUrl,
          true,
          system.map((conflict) => conflict.name),
          true,
          {},
        );
        toast.success(
          t(
            'settings.skills.pluginUpdatedReload',
            'Plugin updated. It applies when a session opens again; type /reload to apply it now.',
          ),
        );
        notifyImportWarnings(result);
        await refreshAfterChange();
      }
    } catch (error) {
      logger.error('Failed to update plugin', error);
      toast.error(errorMessage(error), { id: toastId });
    } finally {
      setIsInstalling(false);
    }
  }, [
    isInstalling,
    pendingUpdate,
    refreshAfterChange,
    runInstall,
    sessionId,
    t,
  ]);

  /** Props for `PluginUpdateNoticeDialog` (#2927 D6). */
  const updateNoticeProps = {
    pendingUpdateUrl: pendingUpdate?.sourceUrl ?? null,
    isBusy: isInstalling,
    onOpenChange: (open: boolean) => {
      if (!open) {
        setPendingUpdate(null);
      }
    },
    onAgree: runUpdateAfterAgree,
  };

  /** Props for `SkillsConflictDialog`. */
  const dialogProps = {
    pendingInstall,
    userConflicts: overwritableConflicts,
    systemConflicts,
    isInstallingLocal: false,
    isInstallingGithub: isInstalling,
    onOpenChange: handleOpenChange,
    onToggleOverwrite: toggleOverwrite,
    onApproveHooksChange: setApproveHooks,
    onUserConfigChange: setPluginOption,
    onConfirm: () => confirm(pendingInstall?.approveHooks ?? false),
    onConfirmWithoutHooks: () => confirm(false),
  };

  return { startInstall, startUpdate, dialogProps, updateNoticeProps };
}
