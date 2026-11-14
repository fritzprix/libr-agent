import { describe, expect, it } from 'vitest';

import { parseInvocation } from '../src/args.js';

const TTY = { stdinIsTty: true, stdoutIsTty: true, stdinText: null };
const PIPE = (text: string) => ({
  stdinIsTty: false,
  stdoutIsTty: true,
  stdinText: text,
});
const NO_TTY = { stdinIsTty: false, stdoutIsTty: false, stdinText: null };

const parse = (argv: string[], input = TTY) =>
  parseInvocation({ argv, ...input });

describe('mode selection (spec 3.2)', () => {
  it('opens the TUI with no arguments in a terminal', () => {
    expect(parse([])).toEqual({ kind: 'tui' });
  });

  it('a positional prompt opens the TUI and sends it first', () => {
    expect(parse(['summarize this'])).toEqual({
      kind: 'tui',
      initialPrompt: 'summarize this',
    });
  });

  it('-p selects headless even in a terminal', () => {
    expect(parse(['-p', 'hi'])).toEqual({ kind: 'headless', prompt: 'hi' });
  });

  it('a piped prompt selects headless', () => {
    expect(parse([], PIPE('from the pipe'))).toEqual({
      kind: 'headless',
      prompt: 'from the pipe',
    });
  });

  it('with -p the piped text becomes context for the prompt', () => {
    const result = parse(['-p', 'summarize'], PIPE('the notes'));
    expect(result.kind).toBe('headless');
    if (result.kind === 'headless')
      expect(result.prompt).toContain('summarize');
    if (result.kind === 'headless')
      expect(result.prompt).toContain('the notes');
  });

  it('an empty pipe is not a prompt', () => {
    expect(parse([], PIPE('   ')).kind).toBe('usage-error');
  });

  it('a positional prompt without a terminal is a usage error telling -p', () => {
    const result = parse(['summarize'], NO_TTY);
    expect(result.kind).toBe('usage-error');
    if (result.kind === 'usage-error') expect(result.message).toContain('-p');
  });

  it('no terminal and no prompt is a usage error', () => {
    expect(parse([], NO_TTY).kind).toBe('usage-error');
  });

  it('--list-sessions works without a terminal', () => {
    expect(parse(['--list-sessions'], NO_TTY)).toEqual({
      kind: 'list-sessions',
    });
  });
});

describe('flags and combinations', () => {
  it('accepts the common session flags', () => {
    expect(
      parse([
        '--cd',
        '/tmp/w',
        '--assistant',
        'a1',
        '--model',
        'm',
        '--provider',
        'p',
        '--isolation',
        'docker',
        '--execution-mode',
        'yolo',
        '--name',
        'job',
      ]),
    ).toEqual({
      kind: 'tui',
      cd: '/tmp/w',
      assistant: 'a1',
      model: 'm',
      provider: 'p',
      isolation: 'docker',
      executionMode: 'yolo',
      name: 'job',
    });
  });

  it('rejects a bad --execution-mode value', () => {
    expect(parse(['--execution-mode', 'reckless']).kind).toBe('usage-error');
  });

  it('rejects unknown flags', () => {
    expect(parse(['--frobnicate']).kind).toBe('usage-error');
  });

  it('-c resumes the most recent session for the directory', () => {
    expect(parse(['-c'])).toEqual({ kind: 'tui', cont: true });
  });

  it('-r with an id resumes it; bare -r opens the picker', () => {
    expect(parse(['-r', 'abc123'])).toEqual({
      kind: 'tui',
      resumeId: 'abc123',
    });
    expect(parse(['--resume'])).toEqual({ kind: 'tui', resumeId: '' });
  });

  it('--continue and --cd cannot be combined', () => {
    expect(parse(['-c', '-C', '/tmp']).kind).toBe('usage-error');
  });

  it('--resume and --cd cannot be combined', () => {
    expect(parse(['-r', 'x', '-C', '/tmp']).kind).toBe('usage-error');
  });

  it('resume flags are rejected in headless mode', () => {
    expect(parse(['-p', 'hi', '-c']).kind).toBe('usage-error');
  });

  it('-p without a value is a usage error', () => {
    expect(parse(['-p']).kind).toBe('usage-error');
  });

  it('help and version win over everything', () => {
    expect(parse(['--help']).kind).toBe('help');
    expect(parse(['--version']).kind).toBe('version');
  });
});
