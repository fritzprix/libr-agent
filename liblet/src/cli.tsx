/**
 * liblet — terminal frontend of the shared LibrAgent backend (issue #2702).
 *
 * This file owns startup: mode selection (args.ts), backend discovery
 * (backend.ts), session resolution (new / --continue / --resume / picker),
 * then handing the session to the interactive view (app.tsx).
 */

import { render } from 'ink';

import { HELP_TEXT, VERSION, EXIT, parseInvocation } from './args.js';
import { NoBackendError, findReadyBackend } from './backend.js';
import { ApiError, SessionApiClient } from './client.js';
import { App } from './app.js';
import { pickSession } from './picker.js';
import { findContinueSession } from './resolve.js';
import type { SessionMetadata } from './types.js';

/** Statuses that mean a running workflow already drives the session. */
const DRIVEN_STATUSES = new Set(['busy', 'queued', 'provisioning']);

/** Piped stdin as text, or null when stdin is a terminal. */
async function readPipedStdin(): Promise<string | null> {
  if (process.stdin.isTTY) return null;
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString('utf8');
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
      // #2703 shipped headless mode as the native Rust `liblet` binary
      // (src-tauri/src/bin/liblet.rs); this Node package is the TUI only.
      console.error(
        'liblet: headless mode lives in the native liblet binary (src-tauri, issue #2703).',
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
  const initialPrompt = parsed.initialPrompt;

  try {
    if (parsed.resumeId !== undefined || parsed.cont) {
      let sessionId = parsed.resumeId;
      if (parsed.cont || sessionId === '') {
        const { sessions } = await client.listSessions();
        if (parsed.cont) {
          const match = findContinueSession(sessions, process.cwd());
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
      // POST /api/sessions/:id/resume (src-tauri/src/server/handlers/sessions.rs:266)
      // has no already-running status — it resets execution state and
      // re-queues the session (src-tauri/src/agent/workflow/pause_resume.rs:30).
      // A session another frontend is driving is therefore detected by its
      // status here: resume is skipped and the TUI header says so. Every
      // other resume failure is a real error and exits 1 — never a silent
      // read-only-looking attach.
      if (!DRIVEN_STATUSES.has(status.status)) await client.resume(sessionId!);
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
      attachedRunning={DRIVEN_STATUSES.has(status.status)}
    />,
    // Ink's default exits on Ctrl+C before useInput sees it; the App
    // decides between interrupting the turn and quitting.
    { exitOnCtrlC: false },
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
