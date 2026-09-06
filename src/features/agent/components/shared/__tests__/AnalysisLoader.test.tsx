import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { PhosphorDotMatrix } from '../PhosphorDotMatrix';
import { AnalysisLoader } from '../AnalysisLoader';


// Mock canvas getContext
beforeEach(() => {
  HTMLCanvasElement.prototype.getContext = vi.fn().mockReturnValue({
    save: vi.fn(),
    restore: vi.fn(),
    scale: vi.fn(),
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    fillStyle: '',
    globalAlpha: 1,
    shadowBlur: 0,
    shadowColor: '',
  }) as unknown as typeof HTMLCanvasElement.prototype.getContext;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('PhosphorDotMatrix', () => {
  it('renders a canvas element with aria-hidden', () => {
    const { container } = render(<PhosphorDotMatrix size="md" />);
    const canvas = container.querySelector('canvas');
    expect(canvas).toBeInTheDocument();
    expect(canvas).toHaveAttribute('aria-hidden', 'true');
  });

  it('handles size variants correctly', () => {
    const { container } = render(<PhosphorDotMatrix size="sm" className="custom-matrix" />);
    const canvas = container.querySelector('canvas');
    expect(canvas).toHaveClass('custom-matrix');
  });
});

describe('AnalysisLoader', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders a valid message from the pool on mount', () => {
    const { container } = render(<AnalysisLoader />);
    const textSpan = container.querySelector('span');
    expect(textSpan).toBeInTheDocument();
    expect(textSpan?.textContent?.trim().length).toBeGreaterThan(0);
  });

  it('advances to next message after interval', () => {
    const { container } = render(<AnalysisLoader />);
    const initialText = container.querySelector('span')?.textContent;
    expect(initialText).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(1900);
    });

    const nextText = container.querySelector('span')?.textContent;
    expect(nextText).toBeTruthy();
    expect(nextText).not.toBe(initialText);
  });


});
