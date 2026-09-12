import { describe, expect, it } from 'vitest';
import {
  canOpenInAppPreview,
  fileNameFromPath,
  isWorkspaceRelativePath,
  PREVIEW_MAX_BYTES,
} from '../filePreview';

describe('fileNameFromPath', () => {
  it('returns the last POSIX or Windows segment', () => {
    expect(fileNameFromPath('src/notes.md')).toBe('notes.md');
    expect(fileNameFromPath('src\\notes.md')).toBe('notes.md');
    expect(fileNameFromPath('notes.md')).toBe('notes.md');
  });
});

describe('isWorkspaceRelativePath', () => {
  it('accepts workspace-relative paths', () => {
    expect(isWorkspaceRelativePath('src/notes.md')).toBe(true);
    expect(isWorkspaceRelativePath('./src/notes.md')).toBe(true);
    expect(isWorkspaceRelativePath('notes.md')).toBe(true);
  });

  it('rejects absolute, home, drive, and traversal paths', () => {
    expect(isWorkspaceRelativePath('/tmp/out.md')).toBe(false);
    expect(isWorkspaceRelativePath('~/out.md')).toBe(false);
    expect(isWorkspaceRelativePath('C:\\tmp\\out.md')).toBe(false);
    expect(isWorkspaceRelativePath('C:/tmp/out.md')).toBe(false);
    expect(isWorkspaceRelativePath('../secret.md')).toBe(false);
    expect(isWorkspaceRelativePath('src/../../etc/passwd')).toBe(false);
    expect(isWorkspaceRelativePath('')).toBe(false);
    expect(isWorkspaceRelativePath('   ')).toBe(false);
  });
});

describe('canOpenInAppPreview', () => {
  it('allows previewable workspace files under the size cap', () => {
    expect(canOpenInAppPreview({ path: 'README.md', size: 100 })).toBe(true);
    expect(canOpenInAppPreview({ path: 'app.tsx', size: 1024 })).toBe(true);
    expect(canOpenInAppPreview({ path: './notes.md' })).toBe(true);
  });

  it('rejects non-previewable types, oversized files, and external paths', () => {
    expect(canOpenInAppPreview({ path: 'report.docx', size: 100 })).toBe(
      false,
    );
    expect(canOpenInAppPreview({ path: 'manual.pdf' })).toBe(false);
    expect(
      canOpenInAppPreview({
        path: 'large.ts',
        size: PREVIEW_MAX_BYTES + 1,
      }),
    ).toBe(false);
    expect(canOpenInAppPreview({ path: '/tmp/out.md', size: 100 })).toBe(
      false,
    );
  });
});
