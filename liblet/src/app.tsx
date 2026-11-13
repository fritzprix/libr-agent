/**
 * The interactive TUI (issue #2702). One shared session view:
 * transcript, composer, inline approvals, and inline user questions —
 * driven entirely by the backend's SSE stream, never by polling loops.
 */

import { useEffect, useReducer, useRef, useState } from 'react';
import { Box, Text, useApp, useInput } from 'ink';

import type { SessionApiClient } from './client.js';
import type {
  ChatMessage,
  PendingApproval,
  SessionStatusResponse,
} from './types.js';
import { initialState, reduce } from './session.js';
import {
  approvalKeyAction,
  askFormPayload,
  askFormReduce,
  initialAskForm,
} from './askform.js';

const MODES = ['normal', 'yolo', 'unsafe'] as const;
const TRANSCRIPT_LINES = 15;

export interface AppProps {
  client: SessionApiClient;
  sessionId: string;
  initialStatus: SessionStatusResponse;
  history: ChatMessage[];
  initialPrompt?: string;
  /** The session was busy/queued at attach; another frontend drives it. */
  attachedRunning?: boolean;
}

function messageText(message: ChatMessage): string {
  return message.content
    .map((block) => {
      if (block.type === 'text' && typeof block.text === 'string')
        return block.text;
      if (block.type === 'toolCall' || block.type === 'tool_use')
        return `[tool] ${block.name ?? 'tool'}`;
      return `[${block.type}]`;
    })
    .join('\n');
}

function roleGlyph(role: string): string {
  if (role === 'user') return '❯';
  if (role === 'assistant') return '◆';
  return '·';
}

export function App({
  client,
  sessionId,
  initialStatus,
  history,
  initialPrompt,
  attachedRunning = false,
}: AppProps) {
  const { exit } = useApp();
  const seed = reduce(
    reduce(initialState(), { type: 'status', status: initialStatus }),
    { type: 'history', messages: history },
  );
  const [state, dispatch] = useReducer(reduce, seed);
  const [draft, setDraft] = useState('');
  const [approvalIndex, setApprovalIndex] = useState(0);
  const [secretDraft, setSecretDraft] = useState('');
  const [askForm, setAskForm] = useState(initialAskForm);
  const sentInitial = useRef(false);

  const prompt = state.pendingUserPrompt;
  const approvals = state.pendingApprovals;
  const running = state.status === 'busy' || state.status === 'queued';
  // The list shrinks as approvals resolve; never index past its end.
  const approvalAt = Math.min(approvalIndex, approvals.length - 1);

  // A new prompt request starts a fresh form (render-phase adjustment).
  const promptId = useRef<string | undefined>(undefined);
  if (prompt?.requestId !== promptId.current) {
    promptId.current = prompt?.requestId;
    setAskForm(initialAskForm());
  }

  // Follow the session's SSE stream for the lifetime of the view.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        for await (const event of client.events(sessionId)) {
          if (cancelled) return;
          if (event.event === 'status')
            dispatch({
              type: 'status',
              status: event.data as never,
            });
          else if (event.event === 'message')
            dispatch({ type: 'message', message: event.data as ChatMessage });
          else if (event.event === 'end') dispatch({ type: 'end' });
        }
        if (!cancelled) dispatch({ type: 'end' });
      } catch (error) {
        if (!cancelled)
          dispatch({
            type: 'notice',
            notice: `event stream ended: ${(error as Error).message}`,
          });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, sessionId]);

  // A positional prompt is sent once, after the stream is attached.
  useEffect(() => {
    if (sentInitial.current || !initialPrompt) return;
    sentInitial.current = true;
    client
      .send(sessionId, initialPrompt)
      .catch((error: Error) =>
        dispatch({ type: 'notice', notice: `send failed: ${error.message}` }),
      );
  }, [client, sessionId, initialPrompt]);

  // The askUser form submits itself once the last question is answered.
  useEffect(() => {
    if (askForm.finished && prompt?.payload.questions)
      respondPrompt(
        'answer',
        JSON.stringify(askFormPayload(askForm, prompt.payload.questions)),
      );
  }, [askForm]);

  const respondPrompt = (action: 'answer' | 'cancel', value?: string) => {
    if (!prompt) return;
    client
      .respondPrompt(sessionId, {
        requestId: prompt.requestId,
        action,
        value,
      })
      .catch((error: Error) =>
        dispatch({ type: 'notice', notice: `prompt failed: ${error.message}` }),
      );
    setSecretDraft('');
    setAskForm(initialAskForm());
  };

  const respondApproval = (approval: PendingApproval, behavior: string) => {
    client
      .respondPermission(sessionId, {
        requestId: approval.requestId,
        toolCallId: approval.toolCallId,
        behavior: behavior as 'allow' | 'deny',
      })
      .catch((error: Error) =>
        dispatch({
          type: 'notice',
          notice: `approval failed: ${error.message}`,
        }),
      );
  };

  const cycleMode = () => {
    const index = MODES.indexOf(state.executionMode as (typeof MODES)[0]);
    const next = MODES[(index + 1) % MODES.length];
    client.setExecutionMode(sessionId, next).catch((error: Error) =>
      dispatch({
        type: 'notice',
        notice: `mode change failed: ${error.message}`,
      }),
    );
  };

  useInput((input, key) => {
    if (key.ctrl && input === 'c') {
      if (running) {
        client
          .terminate(sessionId)
          .then(() => dispatch({ type: 'notice', notice: 'turn interrupted' }))
          .catch((error: Error) =>
            dispatch({ type: 'notice', notice: error.message }),
          );
      } else {
        exit();
      }
      return;
    }

    if (prompt) {
      if (key.escape) {
        respondPrompt('cancel');
        return;
      }
      if (prompt.kind === 'secret') {
        if (key.return) respondPrompt('answer', secretDraft);
        else if (key.backspace || key.delete)
          setSecretDraft((d) => d.slice(0, -1));
        else if (!key.ctrl && !key.meta && input)
          setSecretDraft((d) => d + input);
        return;
      }
      // askUser form: every key rule lives in the pure reducer (askform.ts).
      const questions = prompt.payload.questions ?? [];
      if (questions.length === 0) return;
      setAskForm((prev) =>
        askFormReduce(prev, questions, {
          input,
          returnKey: key.return,
          backspaceKey: key.backspace || key.delete,
        }),
      );
      return;
    }

    if (approvals.length > 0) {
      const action = approvalKeyAction(input, key);
      if (action === 'up') setApprovalIndex((i) => Math.max(0, i - 1));
      else if (action === 'down')
        setApprovalIndex((i) => Math.min(approvals.length - 1, i + 1));
      else if (action === 'allow')
        respondApproval(approvals[approvalAt], 'allow');
      else if (action === 'deny')
        respondApproval(approvals[approvalAt], 'deny');
      else if (action === 'mode') cycleMode();
      return;
    }

    // Composer.
    if (key.return) {
      const text = draft.trim();
      if (text) {
        setDraft('');
        client.send(sessionId, text).catch((error: Error) =>
          dispatch({
            type: 'notice',
            notice: `send failed: ${error.message}`,
          }),
        );
      }
      return;
    }
    if (key.backspace || key.delete) {
      setDraft((d) => d.slice(0, -1));
      return;
    }
    if (!key.ctrl && !key.meta && !key.upArrow && !key.downArrow && input) {
      setDraft((d) => d + input);
    }
  });

  const transcript = state.messages.slice(-TRANSCRIPT_LINES);
  const selected = approvals[approvalAt];
  const questions = prompt?.payload.questions ?? [];
  const question = questions[askForm.questionIndex];
  const answer = question ? askForm.answers[question.id] : undefined;
  const picked = Array.isArray(answer) ? answer : [];

  return (
    <Box flexDirection="column">
      <Text dimColor>
        liblet · {state.sessionName || sessionId.slice(0, 8)} · {state.status}
        {state.statusReason ? ` (${state.statusReason})` : ''}
        {attachedRunning ? ' · driven by another frontend' : ''} ·{' '}
        {state.executionMode} · {state.model ?? ''} ·{' '}
        {state.workspace ?? 'no workspace'}
      </Text>
      <Box flexDirection="column" minHeight={10}>
        {transcript.map((message) => (
          <Text key={message.id} wrap="truncate-end">
            <Text color={message.role === 'user' ? 'cyan' : undefined}>
              {roleGlyph(message.role)}{' '}
            </Text>
            {messageText(message)}
          </Text>
        ))}
      </Box>
      {selected && (
        <Box flexDirection="column" borderStyle="round" borderColor="yellow">
          <Text color="yellow">
            approval needed ({approvalAt + 1}/{approvals.length}) —{' '}
            {selected.approvalKind}
          </Text>
          <Text>
            {selected.toolName}: {selected.commandPreview || selected.reason}
          </Text>
          {approvals.length > 1 && <Text dimColor>↑/↓ select</Text>}
          <Text dimColor>[a] allow [d] deny [m] mode</Text>
        </Box>
      )}
      {prompt && prompt.kind === 'secret' && (
        <Box flexDirection="column" borderStyle="round" borderColor="magenta">
          <Text color="magenta">
            secret needed: {prompt.payload.variableName ?? prompt.payload.label}
          </Text>
          <Text>value: {'•'.repeat(secretDraft.length)}</Text>
          <Text dimColor>enter submit · esc cancel</Text>
        </Box>
      )}
      {prompt && prompt.kind !== 'secret' && question && (
        <Box flexDirection="column" borderStyle="round" borderColor="magenta">
          <Text color="magenta">
            question {askForm.questionIndex + 1}/{questions.length}:{' '}
            {question.question}
          </Text>
          {question.description && <Text dimColor>{question.description}</Text>}
          {(question.options ?? []).map((option, index) => (
            <Text key={option}>
              {index + 1}) {option}
              {picked.includes(option) ? ' ✓' : ''}
            </Text>
          ))}
          {question.options && question.options.length > 0 ? (
            askForm.customMode ? (
              <Text>other ❯ {askForm.pending}</Text>
            ) : (
              <Text dimColor>
                press number
                {question.multiSelect ? ' (multi, then enter)' : ''}
                {question.allowCustom ? ' · o for other' : ''} · enter next
              </Text>
            )
          ) : (
            <Text>❯ {askForm.pending}</Text>
          )}
          <Text dimColor>enter submit · esc cancel all</Text>
        </Box>
      )}
      {state.notice && <Text color="red">{state.notice}</Text>}
      {state.sessionGone && (
        <Text color="red">
          event stream closed — restart liblet to reattach
        </Text>
      )}
      {!prompt && !selected && (
        <Box>
          <Text color="cyan">❯ </Text>
          <Text>{draft}</Text>
          <Text dimColor>▌</Text>
        </Box>
      )}
      <Text dimColor>
        {running ? 'ctrl+c interrupt turn · ' : ''}
        enter send · ctrl+c quit
      </Text>
    </Box>
  );
}
