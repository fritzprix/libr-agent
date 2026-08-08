import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { Message } from '@/models/chat';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import equal from 'fast-deep-equal';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentPanels } from '@/context/AgentPanelsContext';
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

/** Treat missing / empty plan snapshots as equivalent across context reloads. */
function normalizePlanning(
  state: PlanningState | undefined,
): PlanningState | null {
  if (!state) {
    return null;
  }
  if ((state.goal == null || state.goal === '') && state.todos.length === 0) {
    return null;
  }
  return state;
}

function normalizeScratchpad(
  state: ScratchpadState | undefined,
): ScratchpadState | null {
  if (!state) {
    return null;
  }
  if (state.items.length === 0 && state.count === 0) {
    return null;
  }
  return state;
}

export function AgentPlanningUpdates() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { showPlanningPanel } = useAgentPlanning();
  const { markPanelAttention, clearPanelAttention } = useAgentPanels();
  const { serviceContexts, updateServiceContexts } = useAgentChat();
  const previousPlanningRef = useRef<PlanningState | undefined>(undefined);
  const previousScratchpadRef = useRef<ScratchpadState | undefined>(undefined);
  const trackedSessionIdRef = useRef<string | undefined>(undefined);
  /**
   * After a session switch, the next real planning/scratchpad delta is usually
   * the async service-context reload — absorb it instead of notifying.
   */
  const absorbNextDeltaRef = useRef(false);

  const planningState = useMemo(
    () => parsePlanningState(serviceContexts.planning?.structuredState),
    [serviceContexts.planning?.structuredState],
  );
  const scratchpadState = useMemo(
    () => parseScratchpadState(serviceContexts.scratchpad?.structuredState),
    [serviceContexts.scratchpad?.structuredState],
  );

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
    const sessionId = session?.id;

    if (!sessionId) {
      trackedSessionIdRef.current = undefined;
      previousPlanningRef.current = undefined;
      previousScratchpadRef.current = undefined;
      absorbNextDeltaRef.current = false;
      clearPanelAttention('planning');
      return;
    }

    // Session switch (or first mount): adopt current snapshot as baseline and
    // drop leftover attention from the previous session.
    if (trackedSessionIdRef.current !== sessionId) {
      trackedSessionIdRef.current = sessionId;
      previousPlanningRef.current = planningState;
      previousScratchpadRef.current = scratchpadState;
      absorbNextDeltaRef.current = true;
      clearPanelAttention('planning');
      return;
    }

    const planningChanged = !equal(
      normalizePlanning(previousPlanningRef.current),
      normalizePlanning(planningState),
    );
    const scratchpadChanged = !equal(
      normalizeScratchpad(previousScratchpadRef.current),
      normalizeScratchpad(scratchpadState),
    );

    if (!planningChanged && !scratchpadChanged) {
      // Empty→empty after switch: nothing to absorb; arm real updates.
      absorbNextDeltaRef.current = false;
      return;
    }

    const previousTodos = previousPlanningRef.current?.todos;
    previousPlanningRef.current = planningState;
    previousScratchpadRef.current = scratchpadState;

    if (absorbNextDeltaRef.current) {
      absorbNextDeltaRef.current = false;
      return;
    }

    if (!showPlanningPanel) {
      markPanelAttention('planning');
      toast(t('agent.planning.title'), {
        id: `${PLANNING_TOAST_ID_PREFIX}-${sessionId}`,
        duration: 5000,
        description: (
          <PlanningToastSummary
            goal={planningState?.goal ?? null}
            todos={planningState?.todos ?? []}
            previousTodos={previousTodos}
            scratchpad={scratchpadState}
            scratchpadChanged={scratchpadChanged}
          />
        ),
      });
    }
  }, [
    clearPanelAttention,
    markPanelAttention,
    planningState,
    scratchpadState,
    session?.id,
    showPlanningPanel,
    t,
  ]);

  return null;
}
