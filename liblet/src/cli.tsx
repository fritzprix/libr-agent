/**
 * liblet — terminal frontend of the shared LibrAgent backend (issue #2702).
 *
 * This file owns startup: mode selection (args.ts), backend discovery
 * (backend.ts), session resolution (new / --continue / --resume / picker),
 * then handing the session to the interactive view (app.tsx).
 */

import * as path from 'node:path';

import { render } from 'ink';

import { HELP_TEXT, VERSION, EXIT, parseInvocation } from './args.js';
import { NoBackendError, findReadyBackend } from './backend.js';
import { ApiError, SessionApiClient } from './client.js';
import { App } from './app.js';
import { pickSession } from './picker.js';
import type { SessionMetadata } from './types.js';

/** Piped stdin as text, or null when stdin is a terminal. */
async function readPipedStdin(): Promise<string | null> {
  if (process.stdin.isTTY) return null;
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString('utf8');
}

function normalizeWorkspace(p: string): string {
  let out = path.resolve(p).replace(/[\\/]+$/, '');
  if (process.platform === 'win32') out = out.toLowerCase();
  return out;
}

function printSessions(sessions: SessionMetadata[]): void {
  if (sessions.length === 0) {
    console.log('no sessions');
    return;
  }
  for (const s of sessions) {
    const when = new Date(s.lastMessageAt ?? s.updatedAt).toISOString();
    console.log(
      [
        s.id,
        s.status,
        s.executionMode,
        when,
        s.workspaceOverride ?? '',
        s.name ?? '',
      ].join('\t'),
    );
  }
}

async function main(): Promise<number> {
  const stdinText = await readPipedStdin();
  const parsed = parseInvocation({
    argv: process.argv.slice(2),
    stdinText,
    stdinIsTty: process.stdin.isTTY === true,
    stdoutIsTty: process.stdout.isTTY === true,
  });

  switch (parsed.kind) {
    case 'help':
      console.log(HELP_TEXT);
      return EXIT.OK;
    case 'version':
      console.log(`liblet ${VERSION}`);
      return EXIT.OK;
    case 'usage-error':
      console.error(`liblet: ${parsed.message}`);
      console.error('run `liblet --help` for usage');
      return EXIT.USAGE;
    case 'headless':
      console.error(
        'liblet: headless mode is not implemented yet (issue #2703).',
      );
      return EXIT.ERROR;
    case 'list-sessions':
      break;
    case 'tui':
      break;
  }

  let location;
  try {
    location = await findReadyBackend();
  } catch (error) {
    if (error instanceof NoBackendError) {
      console.error(`liblet: ${error.message}`);
      return EXIT.ERROR;
    }
    throw error;
  }
  const client = new SessionApiClient(
    location.port,
    process.env.LIBRAGENT_SESSION_API_KEY,
  );

  if (parsed.kind === 'list-sessions') {
    const { sessions } = await client.listSessions();
    printSessions(sessions);
    return EXIT.OK;
  }

  // TUI path: resolve the session to attach to.
  let status;
  let initialPrompt = parsed.initialPrompt;

  try {
    if (parsed.resumeId !== undefined || parsed.cont) {
      let sessionId = parsed.resumeId;
      if (parsed.cont || sessionId === '') {
        const { sessions } = await client.listSessions();
        if (parsed.cont) {
          const cwd = normalizeWorkspace(process.cwd());
          const match = sessions.find(
            (s) =>
              s.workspaceOverride &&
              normalizeWorkspace(s.workspaceOverride) === cwd,
          );
          if (!match) {
            console.error(`liblet: no session for workspace ${process.cwd()}`);
            return EXIT.ERROR;
          }
          sessionId = match.id;
        } else {
          sessionId = (await pickSession(sessions)) ?? undefined;
          if (!sessionId) return EXIT.OK; // picker cancelled
        }
      }
      status = await client.getSession(sessionId!);
      try {
        await client.resume(sessionId!);
      } catch (error) {
        // Already running elsewhere: attach read-only, one executor per session.
        if (!(error instanceof ApiError)) throw error;
        initialPrompt = undefined;
        process.stderr.write(`liblet: ${error.message}\n`);
      }
    } else {
      const assistants = await client.assistants();
      if (assistants.length === 0) {
        console.error('liblet: the backend has no assistants');
        return EXIT.ERROR;
      }
      status = await client.createSession({
        assistantId: parsed.assistant ?? assistants[0].id,
        name: parsed.name,
        model: parsed.model,
        provider: parsed.provider,
        workspacePath: parsed.cd ?? process.cwd(),
        workspaceIsolation: parsed.isolation,
        executionMode: parsed.executionMode,
      });
    }
  } catch (error) {
    const message =
      error instanceof ApiError || error instanceof Error
        ? error.message
        : String(error);
    console.error(`liblet: ${message}`);
    return EXIT.ERROR;
  }

  const history = await client.messages(status.id);
  const instance = render(
    <App
      client={client}
      sessionId={status.id}
      initialStatus={status}
      history={history.messages}
      initialPrompt={initialPrompt}
    />,
  );
  await instance.waitUntilExit();
  return EXIT.OK;
}

main().then(
  (code) => process.exit(code),
  (error) => {
    console.error(`liblet: ${(error as Error).message}`);
    process.exit(EXIT.ERROR);
  },
);
