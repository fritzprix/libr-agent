import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  renderMermaidSvg,
  resetMermaidLoaderForTests,
} from '../loader';

const renderMock = vi.fn();
const initializeMock = vi.fn();

vi.mock('mermaid', () => ({
  default: {
    initialize: (...args: unknown[]) => initializeMock(...args),
    render: (...args: unknown[]) => renderMock(...args),
  },
}));

describe('renderMermaidSvg', () => {
  beforeEach(() => {
    resetMermaidLoaderForTests();
    initializeMock.mockClear();
    renderMock.mockReset();
    renderMock.mockResolvedValue({ svg: '<svg></svg>' });
  });

  it('initializes once per theme and serializes renders', async () => {
    await Promise.all([
      renderMermaidSvg('flowchart TD\nA-->B', false),
      renderMermaidSvg('flowchart TD\nC-->D', false),
    ]);

    expect(initializeMock).toHaveBeenCalledTimes(1);
    expect(renderMock).toHaveBeenCalledTimes(2);

    await renderMermaidSvg('flowchart TD\nE-->F', true);
    expect(initializeMock).toHaveBeenCalledTimes(2);
    expect(initializeMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ theme: 'dark', securityLevel: 'strict' }),
    );
  });
});
