/**
 * liblet CLI parsing — mode selection and flags per docs/specs/liblet-cli-interface.md
 * sections 3.2/3.3/3.6. Pure function so every mode rule is unit-testable.
 */

export const EXIT = {
  OK: 0,
  ERROR: 1,
  USAGE: 2,
  APPROVAL: 3,
  QUESTION: 4,
  TIMEOUT: 124,
  SIGINT: 130,
  SIGTERM: 143,
} as const;

export const VERSION = '0.1.0';

export const HELP_TEXT = `liblet — terminal frontend for the shared LibrAgent backend

Usage:
  liblet                      Open the TUI on a new session (cwd as workspace)
  liblet "prompt"             Open the TUI and send the prompt first
  echo "text" | liblet        Headless: piped text is the prompt (not yet shipped)
  liblet -p "prompt"          Headless (not yet shipped, issue #2703)
  liblet --list-sessions      Print shared sessions and exit

Common flags:
  -C, --cd <dir>              Workspace for the new session (default: cwd)
  --assistant <id>            Assistant for the new session
  --model <name>              Model for the new session
  --provider <name>           Provider for the new session
  --isolation host|docker     Workspace isolation
  --execution-mode normal|yolo|unsafe   Approval policy at start (default normal)
  --version, --help

TUI only:
  -c, --continue              Resume the most recent session for this directory
  -r, --resume [id]           Resume a shared session (no id: pick one)
  --name <name>               Display name for a new session
`;

export interface ParseInput {
  /** Arguments after the program name. */
  argv: string[];
  /** Full text read from piped stdin, or null when stdin is a terminal. */
  stdinText: string | null;
  stdinIsTty: boolean;
  stdoutIsTty: boolean;
}

export type TuiInvocation = {
  kind: 'tui';
  initialPrompt?: string;
  cont?: boolean;
  /** '' means the resume picker (no id given). */
  resumeId?: string;
  name?: string;
  cd?: string;
  assistant?: string;
  model?: string;
  provider?: string;
  isolation?: string;
  executionMode?: string;
};

export type ParsedInvocation =
  | { kind: 'help' }
  | { kind: 'version' }
  | { kind: 'list-sessions' }
  | { kind: 'headless'; prompt: string }
  | TuiInvocation
  | { kind: 'usage-error'; message: string };

const ISOLATIONS = new Set(['host', 'docker']);
const EXECUTION_MODES = new Set(['normal', 'yolo', 'unsafe']);

export function parseInvocation(input: ParseInput): ParsedInvocation {
  const { argv, stdinText, stdinIsTty, stdoutIsTty } = input;

  let promptFlag: string | undefined;
  let positional: string | undefined;
  let cont = false;
  let resume: string | undefined; // '' = picker
  let name: string | undefined;
  let cd: string | undefined;
  let assistant: string | undefined;
  let model: string | undefined;
  let provider: string | undefined;
  let isolation: string | undefined;
  let executionMode: string | undefined;
  let listSessions = false;

  const needValue = (flag: string): string | undefined => {
    const i = argv.indexOf(flag);
    const value = argv[i + 1];
    if (value === undefined || value.startsWith('-')) return undefined;
    return value;
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case '-h':
      case '--help':
        return { kind: 'help' };
      case '--version':
        return { kind: 'version' };
      case '--list-sessions':
        listSessions = true;
        continue;
      case '-p':
      case '--prompt': {
        const value = needValue(arg);
        if (value === undefined)
          return { kind: 'usage-error', message: `${arg} needs a prompt` };
        promptFlag = value;
        i++;
        continue;
      }
      case '-c':
      case '--continue':
        cont = true;
        continue;
      case '-r':
      case '--resume': {
        const value = needValue(arg);
        resume = value ?? '';
        if (value !== undefined) i++;
        continue;
      }
      case '--name': {
        const value = needValue(arg);
        if (value === undefined)
          return { kind: 'usage-error', message: '--name needs a value' };
        name = value;
        i++;
        continue;
      }
      case '-C':
      case '--cd': {
        const value = needValue(arg);
        if (value === undefined)
          return { kind: 'usage-error', message: `${arg} needs a directory` };
        cd = value;
        i++;
        continue;
      }
      case '--assistant':
      case '--model':
      case '--provider': {
        const value = needValue(arg);
        if (value === undefined)
          return { kind: 'usage-error', message: `${arg} needs a value` };
        if (arg === '--assistant') assistant = value;
        if (arg === '--model') model = value;
        if (arg === '--provider') provider = value;
        i++;
        continue;
      }
      case '--isolation': {
        const value = needValue(arg);
        if (value === undefined || !ISOLATIONS.has(value))
          return {
            kind: 'usage-error',
            message: '--isolation must be host or docker',
          };
        isolation = value;
        i++;
        continue;
      }
      case '--execution-mode': {
        const value = needValue(arg);
        if (value === undefined || !EXECUTION_MODES.has(value))
          return {
            kind: 'usage-error',
            message: '--execution-mode must be normal, yolo or unsafe',
          };
        executionMode = value;
        i++;
        continue;
      }
      default: {
        if (arg.startsWith('-'))
          return { kind: 'usage-error', message: `unknown flag: ${arg}` };
        if (positional !== undefined)
          return { kind: 'usage-error', message: 'only one prompt is allowed' };
        positional = arg;
        continue;
      }
    }
  }

  if (cont && cd)
    return {
      kind: 'usage-error',
      message: '--continue cannot be combined with --cd',
    };
  if (resume !== undefined && cd)
    return {
      kind: 'usage-error',
      message: '--resume cannot be combined with --cd',
    };

  if (listSessions) return { kind: 'list-sessions' };

  // Mode selection (spec 3.2), exactly three outcomes.
  const pipedPrompt =
    stdinText !== null && stdinText.trim() !== '' ? stdinText : undefined;

  if (promptFlag !== undefined || (pipedPrompt && positional === undefined)) {
    const headlessPrompt =
      promptFlag !== undefined
        ? pipedPrompt
          ? `${promptFlag}\n\nPiped input:\n${pipedPrompt}`
          : promptFlag
        : pipedPrompt!;
    if (cont || resume !== undefined)
      return {
        kind: 'usage-error',
        message: '--continue/--resume are not available in headless mode',
      };
    return { kind: 'headless', prompt: headlessPrompt };
  }

  if (positional !== undefined && !(stdinIsTty && stdoutIsTty))
    return {
      kind: 'usage-error',
      message:
        'a positional prompt needs a terminal; add -p for a headless run',
    };

  if (stdinIsTty && stdoutIsTty) {
    const tui: TuiInvocation = { kind: 'tui' };
    if (positional !== undefined) tui.initialPrompt = positional;
    if (cont) tui.cont = true;
    if (resume !== undefined) tui.resumeId = resume;
    if (name !== undefined) tui.name = name;
    if (cd !== undefined) tui.cd = cd;
    if (assistant !== undefined) tui.assistant = assistant;
    if (model !== undefined) tui.model = model;
    if (provider !== undefined) tui.provider = provider;
    if (isolation !== undefined) tui.isolation = isolation;
    if (executionMode !== undefined) tui.executionMode = executionMode;
    return tui;
  }

  return {
    kind: 'usage-error',
    message: 'no terminal: pipe a prompt or use -p for a headless run',
  };
}
