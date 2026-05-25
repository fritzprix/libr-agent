import { type MutableRefObject } from 'react';
import type { Message } from '@/models/chat';
import type { useAgentChat } from '@/context/AgentChatContext';
import type { useAgentSession } from '@/context/AgentSessionContext';

export const INITIAL_FIRST_ITEM_INDEX = 10_000;
export const CHAT_COMPOSER_CLEARANCE = 24;
export const SCROLL_TO_LATEST_BUTTON_OFFSET = CHAT_COMPOSER_CLEARANCE + 16;
// Visual bottom stays intentionally strict so the FAB only hides when the
// viewport is truly pinned.
export const VISUAL_BOTTOM_THRESHOLD = 4;
// Treat three explicit upward scroll gestures as "the user is reading history,
// stop force-following the latest stream". The test harness uses ~12px per
// gesture, so 36px is the practical threshold here.
export const BOTTOM_FOLLOW_RELEASE_SCROLL_DISTANCE = 36;
// Ignore scroll events caused by our own bottom-forcing scroll for one short
// window so programmatic movement does not look like user intent.
export const SELF_SCROLL_IGNORE_WINDOW_MS = 160;

export type BottomAlignmentPhase =
  | 'idle'
  | 'requesting'
  | 'verifying'
  | 'aligned'
  | 'aborted';

export interface AgentChatVirtuosoContext {
  agentError: ReturnType<typeof useAgentChat>['error'];
  agentLlmError: ReturnType<typeof useAgentChat>['llmError'];
  footerEndRef: MutableRefObject<HTMLDivElement | null>;
  hasOlderMessages: boolean;
  isLoadingOlderMessages: boolean;
  latestMessage: Message | undefined;
  loadingOlderLabel: string;
  pendingApprovals: ReturnType<typeof useAgentSession>['pendingApprovals'];
  respondToToolApproval: ReturnType<
    typeof useAgentSession
  >['respondToToolApproval'];
  retryMessage: ReturnType<typeof useAgentChat>['retryMessage'];
  scrollToLoadOlderLabel: string;
  sessionAssistantName: string;
  workflowStatus: ReturnType<typeof useAgentChat>['workflowStatus'];
  yoloModeEnabled: ReturnType<typeof useAgentSession>['yoloModeEnabled'];
}

export type AgentChatVirtuosoContextProps = {
  context: AgentChatVirtuosoContext;
};
