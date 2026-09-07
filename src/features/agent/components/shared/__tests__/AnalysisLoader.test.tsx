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

  it('pins the translated initial message for the first eye-landing beat', () => {
    const { container } = render(<AnalysisLoader />);
    expect(container.querySelector('span')?.textContent).toBe(
      'Preparing response...',
    );
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

  it('keeps rotating messages after the late phase instead of freezing', () => {
    const { container } = render(<AnalysisLoader />);
    const seen = new Set<string>();

    for (let i = 0; i < 80; i += 1) {
      const text = container.querySelector('span')?.textContent ?? '';
      if (text.length > 0) {
        seen.add(text);
      }
      act(() => {
        vi.advanceTimersByTime(1500);
      });
    }

    // Curated witty + late alone are ~19 lines; combinatorial endless should exceed that.
    expect(seen.size).toBeGreaterThan(20);

    const beforeWrap = container.querySelector('span')?.textContent;
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    const afterWrap = container.querySelector('span')?.textContent;
    expect(afterWrap).toBeTruthy();
    expect(afterWrap).not.toBe(beforeWrap);
  });
});
