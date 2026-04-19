import React, { useState } from 'react';
import { SkillsListModal } from '@/features/settings/components/SkillsListModal';
import { cn } from '@/lib/utils';
import { SkillsConflictDialog } from './components/SkillsConflictDialog';
import { SkillsManagementPanelContent } from './components/SkillsManagementPanelContent';
import { SkillsResetDialog } from './components/SkillsResetDialog';
import { useSkillsManagementPanel } from './hooks/useSkillsManagementPanel';

interface SkillsManagementPanelProps {
  className?: string;
}

function SkillsManagementPanelComponent({
  className,
}: SkillsManagementPanelProps) {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const {
    dropZoneRef,
    verificationStatus,
    statusMessage,
    systemSkills,
    userSkills,
    systemDirectory,
    userDirectory,
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
    handleDeleteUserSkill,
    handleResetUserSkills,
  } = useSkillsManagementPanel();

  return (
    <div className={cn('space-y-6', className)}>
      <SkillsManagementPanelContent
        dropZoneRef={dropZoneRef}
        isDragging={isDragging}
        openingDirectory={openingDirectory}
        repoUrl={repoUrl}
        verificationStatus={verificationStatus}
        statusMessage={statusMessage}
        systemDirectory={systemDirectory}
        userDirectory={userDirectory}
        systemSkills={systemSkills}
        userSkills={userSkills}
        isInstallingGithub={isInstallingGithub}
        isInstallingLocal={isInstallingLocal}
        isResettingUserSkills={isResettingUserSkills}
        onRepoUrlChange={setRepoUrl}
        onRefresh={() => void refresh()}
        onViewInstalled={() => setIsModalOpen(true)}
        onOpenDirectory={handleOpenDirectory}
        onOpenResetDialog={() => setIsResetDialogOpen(true)}
        onGitHubInstall={handleGitHubInstall}
      />

      <SkillsListModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        systemSkills={systemSkills}
        userSkills={userSkills}
        deletingSkillName={isDeletingSkill}
        onDeleteUserSkill={(skillName) => void handleDeleteUserSkill(skillName)}
      />

      <SkillsConflictDialog
        pendingInstall={pendingInstall}
        userConflicts={pendingInstallUserConflicts}
        systemConflicts={pendingInstallSystemConflicts}
        isInstallingLocal={isInstallingLocal}
        isInstallingGithub={isInstallingGithub}
        onOpenChange={(open) => {
          if (!open) {
            setPendingInstall(null);
          }
        }}
        onToggleOverwrite={togglePendingOverwrite}
        onConfirm={confirmPendingInstall}
      />

      <SkillsResetDialog
        open={isResetDialogOpen}
        isResettingUserSkills={isResettingUserSkills}
        onOpenChange={(open) => {
          if (!isResettingUserSkills) {
            setIsResetDialogOpen(open);
          }
        }}
        onConfirm={handleResetUserSkills}
      />
    </div>
  );
}

export const SkillsManagementPanel = React.memo(SkillsManagementPanelComponent);
