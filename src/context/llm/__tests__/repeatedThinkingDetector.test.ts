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

  it('returns null for only three identical repetitions', () => {
    const repeatedSegment = 'a'.repeat(REPEATED_THINKING_MIN_PATTERN_LENGTH);

    expect(detectRepeatedThinkingLoop(repeatedSegment.repeat(3))).toBeNull();
  });

  it('returns null when repetitions are interrupted', () => {
    const repeatedSegment = 'b'.repeat(REPEATED_THINKING_MIN_PATTERN_LENGTH);
    const result = detectRepeatedThinkingLoop(
      `${repeatedSegment}${repeatedSegment}${repeatedSegment}${'c'.repeat(repeatedSegment.length)}`,
    );

    expect(result).toBeNull();
  });

  it('returns null when the repeated segment is shorter than the minimum pattern length', () => {
    const result = detectRepeatedThinkingLoop('short'.repeat(25));

    expect(result).toBeNull();
  });

  it('detects long repeated self-overlap even when the last repetition is truncated', () => {
    const repeatedSegment = 'a'.repeat(300);
    // Trailing fragment must be >= minPatternLength (256) to count as the 4th cycle.
    const truncatedRepeat = repeatedSegment.slice(
      0,
      Math.max(
        REPEATED_THINKING_MIN_PATTERN_LENGTH,
        Math.ceil(repeatedSegment.length * 0.75),
      ),
    );
    const noisyPrefix = 'preface '.repeat(12);
    const input = `${noisyPrefix}${repeatedSegment.repeat(3)}${truncatedRepeat}`;
    const expectedTailLength = Math.min(
      input.length,
      REPEATED_THINKING_TAIL_CHARS,
    );

    const result = detectRepeatedThinkingLoop(input);

    expect(result).toEqual({
      observedTailChars: expectedTailLength,
      patternLength: repeatedSegment.length,
      repetitionCount: REPEATED_THINKING_MIN_REPETITIONS,
    });
  });

  it('fires during streaming once a fourth repetition begins', () => {
    const repeatedBlock =
      ('I will use the `ui__presentInteractive` tool.\n' +
      'I will use `workspace__readFile` to get the content.\n' +
      'I will use `ui__presentInteractive` to display it.\n' +
      'I will ask for feedback.\n' +
      'I will be ready to implement after approval.\n').repeat(2);
    const sample = `intro\n${repeatedBlock.repeat(4)}tail`;

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
      repetitionCount: REPEATED_THINKING_MIN_REPETITIONS,
    });
    expect(
      detectRepeatedThinkingLoop(`intro\n${repeatedBlock.repeat(3)}`),
    ).toBeNull();
  });
});
