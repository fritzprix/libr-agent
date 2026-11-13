/**
 * Pure transcript/session state for the TUI.
 *
 * The SSE stream sends whole stored messages (`message` events replace by
 * id) and full status snapshots (`status` events). This reducer keeps them
 * in one renderable state without any backend knowledge.
 */

import type {
  ChatMessage,
  PendingApproval,
  PendingUserPrompt,
  SessionStatusResponse,
} from './types.js';

export interface TuiState {
  messages: ChatMessage[];
  status: string;
  statusReason?: string | null;
  pendingApprovals: PendingApproval[];
  pendingUserPrompt?: PendingUserPrompt;
  executionMode: string;
  workspace?: string | null;
  sessionName?: string | null;
  model?: string;
  provider?: string;
  sessionGone: boolean;
  notice?: string;
}

export function initialState(seed?: Partial<TuiState>): TuiState {
  return {
    messages: [],
    status: 'unknown',
    pendingApprovals: [],
    executionMode: 'normal',
    sessionGone: false,
    ...seed,
  };
}

export type Action =
  | { type: 'history'; messages: ChatMessage[] }
  | { type: 'message'; message: ChatMessage }
  | { type: 'status'; status: SessionStatusResponse }
  | { type: 'end' }
  | { type: 'notice'; notice: string | undefined };

function sortByTime(messages: ChatMessage[]): ChatMessage[] {
  return [...messages].sort((a, b) => a.createdAt - b.createdAt);
}

export function reduce(state: TuiState, action: Action): TuiState {
  switch (action.type) {
    case 'history':
      return { ...state, messages: sortByTime(action.messages) };
    case 'message': {
      const index = state.messages.findIndex((m) => m.id === action.message.id);
      const messages = [...state.messages];
      if (index >= 0) messages[index] = action.message;
      else messages.push(action.message);
      return { ...state, messages: sortByTime(messages) };
    }
    case 'status': {
      const s = action.status;
      return {
        ...state,
        status: s.status,
        statusReason: s.statusReason,
        pendingApprovals: s.pendingApprovals ?? [],
        pendingUserPrompt: s.pendingUserPrompt,
        executionMode: s.executionMode ?? state.executionMode,
        workspace: s.workspaceOverride ?? state.workspace,
        sessionName: s.name ?? state.sessionName,
        model: s.model ?? state.model,
        provider: s.provider ?? state.provider,
      };
    }
    case 'end':
      return { ...state, sessionGone: true };
    case 'notice':
      return { ...state, notice: action.notice };
  }
}
