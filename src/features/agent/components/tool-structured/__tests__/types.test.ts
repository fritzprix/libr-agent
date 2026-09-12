import { describe, expect, it } from 'vitest';
import {
  canRenderStructuredToolResult,
} from '../ToolStructuredResult';
import {
  parseReportResult,
  parseRunShellResult,
  parseStrReplaceResult,
  parseWriteFileResult,
  resolveStructuredToolKey,
} from '../types';

describe('tool-structured types', () => {
  it('resolveStructuredToolKey normalizes builtin names', () => {
    expect(resolveStructuredToolKey('workspace__writeFile')).toBe(
      'workspace__writeFile',
    );
  });

  it('parseWriteFileResult accepts backend snake_case shape', () => {
    const parsed = parseWriteFileResult({
      path: 'src/a.txt',
      absolute_path: '/tmp/workspace/src/a.txt',
      action: 'created',
      bytes_written: 12,
      lines: 2,
    });
    expect(parsed).toEqual({
      path: 'src/a.txt',
      absolute_path: '/tmp/workspace/src/a.txt',
      action: 'created',
      bytes_written: 12,
      lines: 2,
    });
  });

  it('parseWriteFileResult rejects missing path/action', () => {
    expect(parseWriteFileResult({ action: 'created' })).toBeNull();
    expect(parseWriteFileResult({ path: '/tmp/a.txt' })).toBeNull();
  });

  it('parseStrReplaceResult requires unified_diff', () => {
    expect(
      parseStrReplaceResult({
        path: 'a.ts',
        replacements: 1,
        unified_diff: '-a\n+b\n',
      }),
    ).not.toBeNull();
    expect(
      parseStrReplaceResult({
        path: 'a.ts',
        replacements: 1,
      }),
    ).toBeNull();
  });

  it('parseRunShellResult defaults missing stdout/stderr', () => {
    const parsed = parseRunShellResult({
      command: 'echo hi',
      exit_code: 0,
    });
    expect(parsed).toMatchObject({
      command: 'echo hi',
      exit_code: 0,
      stdout: '',
      stderr: '',
    });
  });

  it('canRenderStructuredToolResult only accepts supported tools', () => {
    expect(
      canRenderStructuredToolResult('workspace__writeFile', {
        path: '/tmp/a.txt',
        action: 'overwritten',
        unified_diff: '-a\n+b\n',
      }),
    ).toBe(true);
    expect(
      canRenderStructuredToolResult('workspace__runShell', {
        command: 'ls',
        exit_code: 0,
        stdout: 'ok',
      }),
    ).toBe(true);
    expect(
      canRenderStructuredToolResult('knowledge__searchKnowledge', {
        results: [],
      }),
    ).toBe(false);
    expect(
      canRenderStructuredToolResult('ui__reportResult', {
        status: 'success',
        criteria: 'all done',
        proof: 'tests pass',
        result: 'worked',
      }),
    ).toBe(true);
  });

  it('parseReportResult validates reportResult payload and deliverables', () => {
    const valid = parseReportResult({
      status: 'success',
      title: 'Task Done',
      criteria: 'acceptance check',
      proof: 'verified',
      result: 'Summary here',
      deliverables: [
        {
          path: 'reports/out.pdf',
          absolute_path: '/abs/reports/out.pdf',
          name: 'out.pdf',
          size_bytes: 2048,
          extension: 'pdf',
          exists: true,
        },
      ],
    });

    expect(valid).not.toBeNull();
    expect(valid?.status).toBe('success');
    expect(valid?.deliverables).toHaveLength(1);
    expect(valid?.deliverables[0].name).toBe('out.pdf');

    // Missing required criteria or proof
    expect(
      parseReportResult({
        status: 'success',
        result: 'missing criteria and proof',
      }),
    ).toBeNull();
  });
});
