import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useSWRConfig } from 'swr';
import { getLogger } from '@/lib/logger';
import {
  installSessionPlugin,
  previewSessionPluginInstall,
} from '@/lib/backend/skills';
import type { PendingInstall } from '@/features/skills/skills-management-types';
import {
  formatImportSuccess,
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

  const handleOpenChange = useCallback((open: boolean) => {
    if (!open) {
      setPendingInstall(null);
    }
  }, []);

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
    onConfirm: () => confirm(pendingInstall?.approveHooks ?? false),
    onConfirmWithoutHooks: () => confirm(false),
  };

  return { startInstall, dialogProps };
}
