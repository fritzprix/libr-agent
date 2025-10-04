import { describe, expect, it, vi } from 'vitest';
import { generateWorkspacePath } from '@/lib/workspace-sync-service';

describe('workspace-sync-service', () => {
  it('sanitizes filenames to prevent path traversal sequences', () => {
    const mockTimestamp = 1759496333982;
    const dateSpy = vi.spyOn(Date, 'now').mockReturnValue(mockTimestamp);

    const result = generateWorkspacePath(
      'docker 보다는 uv가 설치가 간편하지 않아_ 여튼 우선 상세 의존성에 대한 조사를....md',
    );

    expect(result.startsWith(`attachments/${mockTimestamp}_`)).toBe(true);
    expect(result.includes('..')).toBe(false);
    expect(result.endsWith('.md')).toBe(true);

    dateSpy.mockRestore();
  });
});
