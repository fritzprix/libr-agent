import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import { BatchModelUpdateDialog } from '../BatchModelUpdateDialog';
import { useStatusBarModelConfig } from './useStatusBarModelConfig';

export function StatusBarModelControls() {
  const {
    session,
    thinkingEffort,
    canUpdateSessionConfig,
    showBatchDialog,
    pendingModelConfig,
    descendantCount,
    handleThinkingEffortChange,
    handleConfigUpdate,
    closeBatchDialog,
    confirmBatchUpdate,
  } = useStatusBarModelConfig();

  return (
    <>
      <div className="min-w-0 w-full sm:w-auto">
        {session && (
          <AgentModelPicker
            currentModel={session.model}
            currentProvider={session.provider}
            className="w-full sm:w-auto"
            disabled={!canUpdateSessionConfig}
            onConfigUpdate={handleConfigUpdate}
            showThinkingEffort
            thinkingEffort={thinkingEffort}
            onThinkingEffortChange={(effort) => {
              void handleThinkingEffortChange(effort);
            }}
          />
        )}
      </div>

      {showBatchDialog && pendingModelConfig && (
        <BatchModelUpdateDialog
          isOpen={showBatchDialog}
          onClose={closeBatchDialog}
          parentSessionName={session?.name || session?.id?.slice(0, 8) || ''}
          subSessionCount={descendantCount}
          newModel={pendingModelConfig.model}
          newProvider={pendingModelConfig.provider}
          onConfirm={confirmBatchUpdate}
        />
      )}
    </>
  );
}
