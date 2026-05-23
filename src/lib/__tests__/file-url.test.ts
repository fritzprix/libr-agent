import { describe, expect, it } from 'vitest';

import { pathToFileUrl, workspacePathToFileUrl } from '../file-url';

describe('file-url utilities', () => {
  it('builds a standard file URL for Windows drive paths', () => {
    expect(pathToFileUrl('C:\\Users\\SKTelecom\\note.md')).toBe(
      'file:///C:/Users/SKTelecom/note.md',
    );
  });

  it('normalizes extended-length Windows drive paths', () => {
    expect(pathToFileUrl('\\\\?\\C:\\Users\\SKTelecom\\note.md')).toBe(
      'file:///C:/Users/SKTelecom/note.md',
    );
    expect(pathToFileUrl('//?/C:/Users/SKTelecom/note.md')).toBe(
      'file:///C:/Users/SKTelecom/note.md',
    );
  });

  it('normalizes extended-length UNC paths', () => {
    expect(pathToFileUrl('\\\\?\\UNC\\server\\share\\note.md')).toBe(
      'file://server/share/note.md',
    );
  });

  it('builds correct URLs for UNC paths', () => {
    expect(pathToFileUrl('\\\\server\\share\\folder\\note.md')).toBe(
      'file://server/share/folder/note.md',
    );
  });

  it('encodes reserved URL characters in filenames', () => {
    expect(pathToFileUrl('C:\\temp\\report?#1.md')).toBe(
      'file:///C:/temp/report%3F%231.md',
    );
  });

  it('builds attachment URLs from workspace paths with extended prefixes', () => {
    expect(
      workspacePathToFileUrl(
        '\\\\?\\C:\\Users\\SKTelecom\\workspace\\session-1\\',
        '\\attachments\\meeting notes.md',
      ),
    ).toBe(
      'file:///C:/Users/SKTelecom/workspace/session-1/attachments/meeting%20notes.md',
    );
  });
});
