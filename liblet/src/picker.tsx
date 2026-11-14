/**
 * Ink resume picker for `liblet -r` with no session id.
 */

import { useState } from 'react';
import { Box, Text, render, useApp, useInput } from 'ink';

import type { SessionMetadata } from './types.js';

function Picker({
  sessions,
  onDone,
}: {
  sessions: SessionMetadata[];
  onDone: (id: string | null) => void;
}) {
  const { exit } = useApp();
  const [index, setIndex] = useState(0);
  useInput((input, key) => {
    if (input === 'q' || key.escape) {
      onDone(null);
      exit();
    } else if (key.upArrow) setIndex((i) => Math.max(0, i - 1));
    else if (key.downArrow)
      setIndex((i) => Math.min(sessions.length - 1, i + 1));
    else if (key.return) {
      onDone(sessions[index]?.id ?? null);
      exit();
    }
  });
  return (
    <Box flexDirection="column">
      <Text color="cyan">resume which session? (↑/↓, enter, q to quit)</Text>
      {sessions.map((session, i) => (
        <Text key={session.id} color={i === index ? 'white' : undefined}>
          {i === index ? '> ' : '  '}
          {(session.name ?? session.id.slice(0, 8))
            .padEnd(20)
            .slice(0, 20)}{' '}
          {session.status.padEnd(8)} {session.workspaceOverride ?? ''}
        </Text>
      ))}
    </Box>
  );
}

export function pickSession(
  sessions: SessionMetadata[],
): Promise<string | null> {
  if (sessions.length === 0) return Promise.resolve(null);
  return new Promise((resolve) => {
    render(<Picker sessions={sessions} onDone={resolve} />);
  });
}
