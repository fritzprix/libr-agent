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

const MODES = ['normal', 'yolo', 'unsafe'] as const;
const TRANSCRIPT_LINES = 15;

export interface AppProps {
  client: SessionApiClient;
  sessionId: string;
  initialStatus: SessionStatusResponse;
  history: ChatMessage[];
  initialPrompt?: string;
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
  const [questionIndex, setQuestionIndex] = useState(0);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const sentInitial = useRef(false);

  const prompt = state.pendingUserPrompt;
  const approvals = state.pendingApprovals;
  const running = state.status === 'busy' || state.status === 'queued';

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
    setAnswers({});
    setQuestionIndex(0);
  };

  const answerCurrentQuestion = () => {
    if (!prompt?.payload.questions) return;
    const questions = prompt.payload.questions;
    if (questionIndex < questions.length - 1) {
      setQuestionIndex(questionIndex + 1);
      return;
    }
    respondPrompt('answer', JSON.stringify(answers));
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
      // askUser form: single selection per question by number.
      const questions = prompt.payload.questions ?? [];
      const question = questions[questionIndex];
      if (!question) return;
      if (question.options && question.options.length > 0) {
        const digit = Number(input);
        if (
          Number.isInteger(digit) &&
          digit >= 1 &&
          digit <= question.options.length
        ) {
          const choice = question.options[digit - 1];
          setAnswers((prev) => ({ ...prev, [question.id]: choice }));
          if (!question.multiSelect) answerCurrentQuestion();
          return;
        }
        if (question.allowCustom && input === 'o') {
          const custom = answers[`${question.id}:pending`] ?? '';
          if (key.return && custom) {
            setAnswers((prev) => ({ ...prev, [question.id]: custom }));
            answerCurrentQuestion();
          }
          return;
        }
      }
      if (key.return) {
        // Free-text question: the draft doubles as the answer.
        const answer = answers[`${question.id}:pending`] ?? draft;
        if (answer) {
          setAnswers((prev) => ({ ...prev, [question.id]: answer }));
          setDraft('');
          answerCurrentQuestion();
        }
        return;
      }
      if (key.backspace || key.delete) {
        setAnswers((prev) => ({
          ...prev,
          [`${question.id}:pending`]: (
            prev[`${question.id}:pending`] ?? draft
          ).slice(0, -1),
        }));
        return;
      }
      if (!key.ctrl && !key.meta && input) {
        setAnswers((prev) => ({
          ...prev,
          [`${question.id}:pending`]:
            (prev[`${question.id}:pending`] ?? draft) + input,
        }));
      }
      return;
    }

    if (approvals.length > 0) {
      if (key.upArrow) setApprovalIndex((i) => Math.max(0, i - 1));
      else if (key.downArrow)
        setApprovalIndex((i) => Math.min(approvals.length - 1, i + 1));
      else if (input === 'a' || input === 'y')
        respondApproval(approvals[approvalIndex], 'allow');
      else if (input === 'd' || input === 'n')
        respondApproval(approvals[approvalIndex], 'deny');
      else if (input === 'm') cycleMode();
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
  const selected = approvals[approvalIndex];
  const questions = prompt?.payload.questions ?? [];
  const question = questions[questionIndex];
  const typing = answers[`${question?.id}:pending`] ?? (question ? '' : draft);

  return (
    <Box flexDirection="column">
      <Text dimColor>
        liblet · {state.sessionName || sessionId.slice(0, 8)} · {state.status}
        {state.statusReason ? ` (${state.statusReason})` : ''} ·{' '}
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
            approval needed ({approvalIndex + 1}/{approvals.length}) —{' '}
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
            question {questionIndex + 1}/{questions.length}: {question.question}
          </Text>
          {question.description && <Text dimColor>{question.description}</Text>}
          {(question.options ?? []).map((option, index) => (
            <Text key={option}>
              {index + 1}) {option}
              {answers[question.id] === option ? ' ✓' : ''}
            </Text>
          ))}
          {question.options && question.options.length > 0 ? (
            <Text dimColor>
              press number{question.multiSelect ? ' (multi, then enter)' : ''}
              {question.allowCustom ? ' · o then text for other' : ''} · enter
              next
            </Text>
          ) : (
            <Text>❯ {typing}</Text>
          )}
          <Text dimColor>enter submit · esc cancel all</Text>
        </Box>
      )}
      {state.notice && <Text color="red">{state.notice}</Text>}
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
