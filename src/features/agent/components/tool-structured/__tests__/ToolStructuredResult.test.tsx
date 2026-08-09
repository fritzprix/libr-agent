import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ToolStructuredResult } from '../ToolStructuredResult';

vi.mock('@/lib/backend', () => ({
  openPathWithDefaultApp: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('ToolStructuredResult', () => {
  it('renders writeFile structured view with open when absolute_path is present', () => {
    render(
      <ToolStructuredResult
        toolName="workspace__writeFile"
        data={{
          path: 'src/new.ts',
          absolute_path: '/home/user/project/src/new.ts',
          action: 'created',
          bytes_written: 42,
          lines: 3,
        }}
      />,
    );
    expect(screen.getByTestId('tool-structured-write-file')).toBeInTheDocument();
    expect(screen.getByText('src/new.ts')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /open file/i })).toBeInTheDocument();
  });

  it('hides open button when writeFile absolute_path is missing', () => {
    render(
      <ToolStructuredResult
        toolName="workspace__writeFile"
        data={{
          path: 'src/new.ts',
          action: 'created',
          bytes_written: 42,
          lines: 3,
        }}
      />,
    );
    expect(screen.getByTestId('tool-structured-write-file')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /open file/i }),
    ).not.toBeInTheDocument();
  });

  it('renders strReplace diff view', () => {
    render(
      <ToolStructuredResult
        toolName="workspace__strReplace"
        data={{
          path: 'src/a.ts',
          replacements: 1,
          unified_diff: '--- a/src/a.ts\n+++ b/src/a.ts\n-old\n+new\n',
        }}
      />,
    );
    expect(screen.getByTestId('tool-structured-str-replace')).toBeInTheDocument();
    expect(screen.getByText('src/a.ts')).toBeInTheDocument();
    expect(screen.getByText('+new')).toBeInTheDocument();
  });

  it('renders runShell terminal block', () => {
    render(
      <ToolStructuredResult
        toolName="workspace__runShell"
        data={{
          command: 'echo hello',
          exit_code: 0,
          stdout: 'hello\n',
          stderr: '',
          duration_ms: 12,
        }}
      />,
    );
    expect(screen.getByTestId('tool-structured-run-shell')).toBeInTheDocument();
    expect(screen.getByText('echo hello')).toBeInTheDocument();
    expect(screen.getByText('hello')).toBeInTheDocument();
  });

  it('returns null for unsupported or invalid payloads', () => {
    const { container: unsupported } = render(
      <ToolStructuredResult
        toolName="knowledge__searchKnowledge"
        data={{ results: [] }}
      />,
    );
    expect(unsupported.firstChild).toBeNull();

    const { container: invalid } = render(
      <ToolStructuredResult
        toolName="workspace__writeFile"
        data={{ action: 'created' }}
      />,
    );
    expect(invalid.firstChild).toBeNull();
  });
});
