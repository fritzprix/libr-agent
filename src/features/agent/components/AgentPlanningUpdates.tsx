import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { Message } from '@/models/chat';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import equal from 'fast-deep-equal';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentPlanning } from '@/context/AgentPlanningContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import {
  parsePlanningState,
  parseScratchpadState,
  type PlanningState,
  type ScratchpadState,
} from '@/models/planning';
import { PlanningToastSummary } from './PlanningToastSummary';

const logger = getLogger('AgentPlanningUpdates');

const PLANNING_TOAST_ID_PREFIX = 'agent-planning-update';

export function AgentPlanningUpdates() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { showPlanningPanel } = useAgentPlanning();
  const { serviceContexts, updateServiceContexts } = useAgentChat();
  const previousPlanningRef = useRef<PlanningState | undefined>(undefined);
  const previousScratchpadRef = useRef<ScratchpadState | undefined>(undefined);
  const hasHydratedRef = useRef(false);

  const planningState = useMemo(
    () => parsePlanningState(serviceContexts.planning?.structuredState),
    [serviceContexts.planning?.structuredState],
  );
  const scratchpadState = useMemo(
    () => parseScratchpadState(serviceContexts.scratchpad?.structuredState),
    [serviceContexts.scratchpad?.structuredState],
  );

  useEffect(() => {
    if (!session?.id) {
      hasHydratedRef.current = false;
      previousPlanningRef.current = undefined;
      previousScratchpadRef.current = undefined;
    }
  }, [session?.id]);

  const triggerCallback = useCallback(() => {
    updateServiceContexts().catch((error: unknown) => {
      logger.error('Failed to refresh planning contexts after tool update', {
        sessionId: session?.id,
        error,
      });
    });
  }, [session?.id, updateServiceContexts]);

  const triggerOptions = useMemo(
    () => ({
      debounceMs: 200,
      messageFilter: (message: Message) => {
        if (!session?.id || message.sessionId !== session.id) return false;

        // Tool result messages are emitted with role === 'tool' and tool_call_id.
        // Refresh contexts after any successful tool completion so planning and
        // scratchpad state stay in sync even when the backend does not attach
        // tool_use metadata to tool result messages.
        if (
          message.role === 'tool' &&
          typeof message.tool_call_id === 'string' &&
          message.tool_call_id.length > 0 &&
          !message.error &&
          message.metadata?.toolError !== true
        ) {
          return true;
        }

        return false;
      },
    }),
    [session?.id],
  );

  useAgentMessageTrigger(triggerCallback, triggerOptions);

  useEffect(() => {
    if (!session?.id) {
      return;
    }

    if (!hasHydratedRef.current) {
      previousPlanningRef.current = planningState;
      previousScratchpadRef.current = scratchpadState;
      hasHydratedRef.current = true;
      return;
    }

    const planningChanged = !equal(previousPlanningRef.current, planningState);
    const scratchpadChanged = !equal(
      previousScratchpadRef.current,
      scratchpadState,
    );

    if ((planningChanged || scratchpadChanged) && !showPlanningPanel) {
      toast(t('agent.planning.title'), {
        id: `${PLANNING_TOAST_ID_PREFIX}-${session.id}`,
        duration: 5000,
        description: (
          <PlanningToastSummary
            goal={planningState?.goal ?? null}
            todos={planningState?.todos ?? []}
            previousTodos={previousPlanningRef.current?.todos}
            scratchpad={scratchpadState}
            scratchpadChanged={scratchpadChanged}
          />
        ),
      });
    }

    previousPlanningRef.current = planningState;
    previousScratchpadRef.current = scratchpadState;
  }, [planningState, scratchpadState, session?.id, showPlanningPanel, t]);

  return null;
}
