import { describe, expect, it } from 'vitest';

import {
  detectRepeatedThinkingLoop,
  REPEATED_THINKING_MIN_PATTERN_LENGTH,
  REPEATED_THINKING_MIN_REPETITIONS,
  REPEATED_THINKING_TAIL_CHARS,
} from '../repeatedThinkingDetector';

describe('detectRepeatedThinkingLoop', () => {
  it('detects a repeated tail pattern that meets the minimum thresholds', () => {
    const repeatedSegment = '0123456789abcdef'.repeat(
      REPEATED_THINKING_MIN_PATTERN_LENGTH / 16,
    );
    const result = detectRepeatedThinkingLoop(
      repeatedSegment.repeat(REPEATED_THINKING_MIN_REPETITIONS),
    );

    expect(result).toEqual({
      observedTailChars:
        repeatedSegment.length * REPEATED_THINKING_MIN_REPETITIONS,
      patternLength: repeatedSegment.length,
      repetitionCount: REPEATED_THINKING_MIN_REPETITIONS,
    });
  });

  it('returns null when repetitions are interrupted', () => {
    const repeatedSegment = 'b'.repeat(REPEATED_THINKING_MIN_PATTERN_LENGTH);
    const result = detectRepeatedThinkingLoop(
      `${repeatedSegment}${repeatedSegment}${'c'.repeat(repeatedSegment.length)}`,
    );

    expect(result).toBeNull();
  });

  it('returns null when the repeated segment is shorter than the minimum pattern length', () => {
    const result = detectRepeatedThinkingLoop('short'.repeat(12));

    expect(result).toBeNull();
  });

  it('detects long repeated self-overlap even when the last repetition is truncated', () => {
    const repeatedSegment =
      'I will use the `ui__presentInteractive` tool.\n\nThe content is ready.\n\nI will start the response.\n\n';
    const truncatedRepeat = repeatedSegment.slice(0, repeatedSegment.length / 2);
    const noisyPrefix = 'preface '.repeat(12);
    const input = `${noisyPrefix}${repeatedSegment.repeat(2)}${truncatedRepeat}`;
    const expectedTailLength = Math.min(
      input.length,
      REPEATED_THINKING_TAIL_CHARS,
    );

    const result = detectRepeatedThinkingLoop(input);

    expect(result).toEqual({
      observedTailChars: expectedTailLength,
      patternLength: repeatedSegment.length,
      repetitionCount: 2,
    });
  });

  it('fires during streaming before the repeated planning loop completes', () => {
    const repeatedBlock =
      'I will use the `ui__presentInteractive` tool.\n' +
      'I will use `workspace__readFile` to get the content.\n' +
      'I will use `ui__presentInteractive` to display it.\n' +
      'I will ask for feedback.\n' +
      'I will be ready to implement after approval.\n';
    const sample = `intro\n${repeatedBlock}${repeatedBlock}tail`;

    let firstDetection: ReturnType<typeof detectRepeatedThinkingLoop> = null;
    for (let prefixLength = 1; prefixLength <= sample.length; prefixLength++) {
      const detection = detectRepeatedThinkingLoop(sample.slice(0, prefixLength));
      if (detection) {
        firstDetection = detection;
        break;
      }
    }

    expect(firstDetection).not.toBeNull();
    expect(firstDetection).toMatchObject({
      patternLength: repeatedBlock.length,
      repetitionCount: 2,
    });
  });
});
