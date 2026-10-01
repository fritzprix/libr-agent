import { useAgentChat } from '@/context/AgentChatContext';
import { useBusyComposerTip, WaitTipLink } from '@/features/spotlight';

/**
 * Non-intrusive feature tip strip above the chat input while workflow is busy.
 * Keeps AnalysisLoader jokes untouched in the message list.
 */
export function ComposerBusyTipStrip() {
  const { workflowStatus } = useAgentChat();
  const tip = useBusyComposerTip(workflowStatus === 'busy');

  if (!tip) {
    return null;
  }

  return (
    <div
      className="mb-2 flex min-w-0 items-center px-1"
      data-testid="composer-busy-tip-strip"
    >
      <WaitTipLink tip={tip} />
    </div>
  );
}
