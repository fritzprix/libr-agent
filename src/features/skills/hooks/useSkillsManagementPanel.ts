import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useSWRConfig } from 'swr';
import { useDnDContext } from '@/context/DnDContext';
import { safeInvoke } from '@/lib/backend/core';
import {
  deletePluginSource,
  deleteUserSkill,
  importUserSkills,
  installGitHubSkills,
  previewGitHubSkillInstall,
  previewUserSkillImport,
  resetUserSkills,
} from '@/lib/backend/skills';
import { getLogger } from '@/lib/logger';
import { useSkillsDirectory } from '@/features/settings/hooks/useSkillsDirectory';
import { isScopedSkillsKey } from '@/features/agent/hooks/useScopedSkills';
import {
  defaultPluginOptions,
  formatImportSuccess,
  getSkillsStatusMessage,
  notifyImportWarnings,
  shouldPromptSkillInstall,
} from '../skills-management-utils';
import type {
  PendingInstall,
  SkillsDirectoryScope,
} from '../skills-management-types';

const logger = getLogger('useSkillsManagementPanel');

export function useSkillsManagementPanel() {
  const { t } = useTranslation('common');
  const { subscribe } = useDnDContext();
  const {
    verificationStatus,
    skills,
    systemSkills,
    userSkills,
    systemDirectory,
    userDirectory,
    pluginSources,
    errorMessage,
    refresh: refreshOverview,
  } = useSkillsDirectory();
  const { mutate } = useSWRConfig();

  // Skill changes must also reach @skill: autocomplete lists that are already open.
  const refresh = useCallback(async () => {
    await Promise.all([refreshOverview(), mutate(isScopedSkillsKey)]);
  }, [refreshOverview, mutate]);

  const dropZoneRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [pendingInstall, setPendingInstall] = useState<PendingInstall | null>(
    null,
  );
  const [isResetDialogOpen, setIsResetDialogOpen] = useState(false);
  const [isInstallingLocal, setIsInstallingLocal] = useState(false);
  const [isInstallingGithub, setIsInstallingGithub] = useState(false);
  const [isDeletingSkill, setIsDeletingSkill] = useState<string | null>(null);
  const [isResettingUserSkills, setIsResettingUserSkills] = useState(false);
  const [openingDirectory, setOpeningDirectory] =
    useState<SkillsDirectoryScope | null>(null);
  const [repoUrl, setRepoUrl] = useState('');

  const pendingInstallSystemConflicts = useMemo(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'system',
      ) ?? [],
    [pendingInstall],
  );

  const pendingInstallUserConflicts = useMemo(
    () =>
      pendingInstall?.preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'user',
      ) ?? [],
    [pendingInstall],
  );

  const performInstall = useCallback(
    async (
      kind: PendingInstall['kind'],
      sourceValue: string,
      overwriteNames: string[],
      approveHooks: boolean,
      pluginOptions: Record<string, string> = {},
      extraExcludedNames: string[] = [],
    ) => {
      const userConflictNames = new Set(
        pendingInstallUserConflicts.map((conflict) => conflict.name),
      );
      const excludedNames = [
        ...pendingInstallSystemConflicts.map((conflict) => conflict.name),
        ...pendingInstallUserConflicts
          .map((conflict) => conflict.name)
          .filter((name) => !overwriteNames.includes(name)),
        ...extraExcludedNames,
      ].filter((name, index, names) => names.indexOf(name) === index);
      const shouldOverwrite = overwriteNames.some((name) =>
        userConflictNames.has(name),
      );

      return kind === 'local'
        ? importUserSkills(
            sourceValue,
            shouldOverwrite,
            excludedNames,
            approveHooks,
          )
        : installGitHubSkills(
            sourceValue,
            shouldOverwrite,
            excludedNames,
            approveHooks,
            pluginOptions,
          );
    },
    [pendingInstallSystemConflicts, pendingInstallUserConflicts],
  );

  useEffect(() => {
    if (!dropZoneRef.current) {
      return;
    }

    return subscribe(
      dropZoneRef,
      async (event, payload) => {
        if (event === 'drag-over') {
          if (payload.paths && payload.paths.length > 0) {
            setIsDragging(true);
          }
          return;
        }

        if (event === 'leave') {
          setIsDragging(false);
          return;
        }

        setIsDragging(false);
        const filePath = payload.paths?.[0];
        if (!filePath || isInstallingLocal || isInstallingGithub) {
          return;
        }

        const toastId = toast.loading(
          t(
            'settings.skills.importingUserSkills',
            'Inspecting dropped skill package...',
          ),
        );

        try {
          const preview = await previewUserSkillImport(filePath);
          toast.dismiss(toastId);
          const userConflicts = preview.conflicts.filter(
            (conflict) => conflict.existingOrigin === 'user',
          );
          if (shouldPromptSkillInstall(preview, userConflicts.length)) {
            setPendingInstall({
              kind: 'local',
              sourceValue: filePath,
              preview,
              selectedOverwriteNames: userConflicts.map(
                (conflict) => conflict.name,
              ),
              approveHooks: false,
              pluginOptions: defaultPluginOptions(preview),
            });
            return;
          }

          setIsInstallingLocal(true);
          const result = await importUserSkills(
            filePath,
            false,
            preview.conflicts
              .filter((conflict) => conflict.existingOrigin === 'system')
              .map((conflict) => conflict.name),
            false,
          );
          toast.success(
            formatImportSuccess(
              t,
              result.importedNames.length,
              result.overwrittenNames.length,
              result.skippedNames.length,
            ),
          );
          notifyImportWarnings(result);
          await refresh();
        } catch (error) {
          logger.error('Failed to import dropped skills', error);
          toast.error(error instanceof Error ? error.message : String(error), {
            id: toastId,
          });
        } finally {
          setIsInstallingLocal(false);
        }
      },
      { priority: 2 },
    );
  }, [isInstallingGithub, isInstallingLocal, refresh, subscribe, t]);

  const handleOpenDirectory = useCallback(
    async (directory: string, scope: SkillsDirectoryScope) => {
      if (!directory || openingDirectory) {
        return;
      }

      setOpeningDirectory(scope);
      try {
        await safeInvoke<void>('open_skills_directory_in_explorer', {
          directory,
        });
      } catch (error) {
        logger.error(`Failed to open ${scope} skills directory`, error);
        toast.error(error instanceof Error ? error.message : String(error));
      } finally {
        setOpeningDirectory(null);
      }
    },
    [openingDirectory],
  );

  const handleGitHubInstall = useCallback(async () => {
    const trimmedRepoUrl = repoUrl.trim();
    if (!trimmedRepoUrl || isInstallingGithub || isInstallingLocal) {
      return;
    }

    setIsInstallingGithub(true);
    const toastId = toast.loading(
      t('settings.skills.githubInspecting', 'Inspecting GitHub repository...'),
    );

    try {
      const preview = await previewGitHubSkillInstall(trimmedRepoUrl);
      toast.dismiss(toastId);
      const userConflicts = preview.conflicts.filter(
        (conflict) => conflict.existingOrigin === 'user',
      );
      if (shouldPromptSkillInstall(preview, userConflicts.length)) {
        setPendingInstall({
          kind: 'github',
          sourceValue: trimmedRepoUrl,
          preview,
          selectedOverwriteNames: userConflicts.map(
            (conflict) => conflict.name,
          ),
          approveHooks: false,
          pluginOptions: defaultPluginOptions(preview),
        });
        return;
      }

      const result = await installGitHubSkills(
        trimmedRepoUrl,
        false,
        preview.conflicts
          .filter((conflict) => conflict.existingOrigin === 'system')
          .map((conflict) => conflict.name),
        false,
      );
      toast.success(
        formatImportSuccess(
          t,
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      notifyImportWarnings(result);
      setRepoUrl('');
      await refresh();
    } catch (error) {
      logger.error('Failed to install skills from GitHub', error);
      toast.error(error instanceof Error ? error.message : String(error), {
        id: toastId,
      });
    } finally {
      setIsInstallingGithub(false);
    }
  }, [isInstallingGithub, isInstallingLocal, refresh, repoUrl, t]);

  const confirmPendingInstall = useCallback(async () => {
    if (!pendingInstall) {
      return;
    }

    const { kind, sourceValue } = pendingInstall;
    setIsInstallingLocal(kind === 'local');
    setIsInstallingGithub(kind === 'github');

    try {
      const result = await performInstall(
        kind,
        sourceValue,
        pendingInstall.selectedOverwriteNames,
        pendingInstall.approveHooks,
        pendingInstall.pluginOptions,
      );
      toast.success(
        formatImportSuccess(
          t,
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      notifyImportWarnings(result);
      if (kind === 'github') {
        setRepoUrl('');
      }
      setPendingInstall(null);
      await refresh();
    } catch (error) {
      logger.error('Failed to confirm skill install overwrite', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsInstallingLocal(false);
      setIsInstallingGithub(false);
    }
  }, [pendingInstall, performInstall, refresh, t]);

  const togglePendingOverwrite = useCallback((skillName: string) => {
    setPendingInstall((current) => {
      if (!current) {
        return current;
      }

      return {
        ...current,
        selectedOverwriteNames: current.selectedOverwriteNames.includes(
          skillName,
        )
          ? current.selectedOverwriteNames.filter((name) => name !== skillName)
          : [...current.selectedOverwriteNames, skillName],
      };
    });
  }, []);

  const setPendingApproveHooks = useCallback((approveHooks: boolean) => {
    setPendingInstall((current) =>
      current ? { ...current, approveHooks } : current,
    );
  }, []);

  /** Record one `userConfig` answer (#2926). A sensitive answer stays in this
   *  in-memory dialog state and is sent straight to secure storage. */
  const setPendingPluginOption = useCallback((key: string, value: string) => {
    setPendingInstall((current) =>
      current
        ? {
            ...current,
            pluginOptions: { ...current.pluginOptions, [key]: value },
          }
        : current,
    );
  }, []);

  const confirmPendingInstallWithoutHooks = useCallback(async () => {
    if (!pendingInstall) {
      return;
    }

    const { kind, sourceValue } = pendingInstall;
    setIsInstallingLocal(kind === 'local');
    setIsInstallingGithub(kind === 'github');

    try {
      const result = await performInstall(
        kind,
        sourceValue,
        pendingInstall.selectedOverwriteNames,
        false,
        pendingInstall.pluginOptions,
      );
      toast.success(
        formatImportSuccess(
          t,
          result.importedNames.length,
          result.overwrittenNames.length,
          result.skippedNames.length,
        ),
      );
      notifyImportWarnings(result);
      if (kind === 'github') {
        setRepoUrl('');
      }
      setPendingInstall(null);
      await refresh();
    } catch (error) {
      logger.error('Failed to install skills without hook approval', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsInstallingLocal(false);
      setIsInstallingGithub(false);
    }
  }, [pendingInstall, performInstall, refresh, t]);

  const handleReinstallPluginSource = useCallback(
    async (sourceUrl: string) => {
      if (isInstallingGithub || isInstallingLocal) {
        return;
      }
      setIsInstallingGithub(true);
      const toastId = toast.loading(
        t(
          'settings.skills.githubInspecting',
          'Inspecting GitHub repository...',
        ),
      );
      try {
        const preview = await previewGitHubSkillInstall(sourceUrl);
        toast.dismiss(toastId);
        const userConflicts = preview.conflicts.filter(
          (c) => c.existingOrigin === 'user',
        );
        if (shouldPromptSkillInstall(preview, userConflicts.length)) {
          setPendingInstall({
            kind: 'github',
            sourceValue: sourceUrl,
            preview,
            selectedOverwriteNames: userConflicts.map((c) => c.name),
            approveHooks: false,
            pluginOptions: defaultPluginOptions(preview),
          });
          return;
        }
        await installGitHubSkills(
          sourceUrl,
          true,
          preview.conflicts
            .filter((c) => c.existingOrigin === 'system')
            .map((c) => c.name),
          false,
        );
        toast.success(
          t('settings.skills.pluginSourceUpdated', 'Plugin updated.'),
        );
        await refresh();
      } catch (error) {
        logger.error('Failed to update plugin source', error);
        toast.error(error instanceof Error ? error.message : String(error), {
          id: toastId,
        });
      } finally {
        setIsInstallingGithub(false);
      }
    },
    [isInstallingGithub, isInstallingLocal, refresh, t],
  );

  const handleDeletePluginSource = useCallback(
    async (sourceId: string) => {
      if (isDeletingSkill) {
        return;
      }
      setIsDeletingSkill(sourceId);
      try {
        await deletePluginSource(sourceId);
        toast.success(
          t('settings.skills.pluginSourceRemoved', 'Plugin source removed.'),
        );
        await refresh();
      } catch (error) {
        logger.error('Failed to delete plugin source', error);
        toast.error(error instanceof Error ? error.message : String(error));
      } finally {
        setIsDeletingSkill(null);
      }
    },
    [isDeletingSkill, refresh, t],
  );

  const handleDeleteUserSkill = useCallback(
    async (skillName: string) => {
      if (isDeletingSkill) {
        return;
      }

      setIsDeletingSkill(skillName);
      try {
        await deleteUserSkill(skillName);
        toast.success(
          t('settings.skills.userSkillDeleted', 'User skill deleted.'),
        );
        await refresh();
      } catch (error) {
        logger.error('Failed to delete user skill', error);
        toast.error(error instanceof Error ? error.message : String(error));
      } finally {
        setIsDeletingSkill(null);
      }
    },
    [isDeletingSkill, refresh, t],
  );

  const handleResetUserSkills = useCallback(async () => {
    if (isResettingUserSkills) {
      return;
    }

    setIsResettingUserSkills(true);
    try {
      await resetUserSkills();
      toast.success(t('settings.skills.userSkillsReset', 'User skills reset.'));
      setIsResetDialogOpen(false);
      await refresh();
    } catch (error) {
      logger.error('Failed to reset user skills', error);
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsResettingUserSkills(false);
    }
  }, [isResettingUserSkills, refresh, t]);

  const statusMessage = useMemo(
    () =>
      getSkillsStatusMessage(
        verificationStatus,
        errorMessage,
        skills.length,
        t,
      ),
    [errorMessage, skills.length, t, verificationStatus],
  );

  return {
    dropZoneRef,
    verificationStatus,
    statusMessage,
    skills,
    systemSkills,
    userSkills,
    systemDirectory,
    userDirectory,
    pluginSources,
    isDragging,
    pendingInstall,
    pendingInstallSystemConflicts,
    pendingInstallUserConflicts,
    isResetDialogOpen,
    isInstallingLocal,
    isInstallingGithub,
    isDeletingSkill,
    isResettingUserSkills,
    openingDirectory,
    repoUrl,
    setPendingInstall,
    setIsResetDialogOpen,
    setRepoUrl,
    refresh,
    handleOpenDirectory,
    handleGitHubInstall,
    confirmPendingInstall,
    togglePendingOverwrite,
    setPendingApproveHooks,
    setPendingPluginOption,
    confirmPendingInstallWithoutHooks,
    handleReinstallPluginSource,
    handleDeletePluginSource,
    handleDeleteUserSkill,
    handleResetUserSkills,
  };
}
