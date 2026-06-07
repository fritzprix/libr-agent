import { describe, expect, it } from 'vitest';

import {
  detectRepeatedTextLoop,
  detectRepeatedThinkingLoop,
  detectRepeatedTailLoop,
  REPEATED_TEXT_MIN_PATTERN_LENGTH,
  REPEATED_TEXT_MIN_REPETITIONS,
  REPEATED_THINKING_MIN_PATTERN_LENGTH,
  REPEATED_THINKING_MIN_REPETITIONS,
  REPEATED_THINKING_TAIL_CHARS,
} from '../repeatedTailDetector';

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
});

describe('detectRepeatedTextLoop', () => {
  it('requires three repetitions before firing', () => {
    const repeatedSegment = 'a'.repeat(REPEATED_TEXT_MIN_PATTERN_LENGTH);
    const twoRepeats = repeatedSegment.repeat(2);

    expect(detectRepeatedTextLoop(twoRepeats)).toBeNull();

    const threeRepeats = repeatedSegment.repeat(REPEATED_TEXT_MIN_REPETITIONS);
    const result = detectRepeatedTextLoop(threeRepeats);

    expect(result).toEqual({
      observedTailChars: threeRepeats.length,
      patternLength: repeatedSegment.length,
      repetitionCount: REPEATED_TEXT_MIN_REPETITIONS,
    });
  });

  it('does not treat short enumerated lines as a loop at the text threshold', () => {
    const line = '- item one: do the thing\n';
    const listLike = line.repeat(8);

    expect(
      detectRepeatedTailLoop(listLike, {
        minPatternLength: REPEATED_TEXT_MIN_PATTERN_LENGTH,
        minRepetitions: REPEATED_TEXT_MIN_REPETITIONS,
        tailChars: REPEATED_THINKING_TAIL_CHARS,
      }),
    ).toBeNull();
  });

  it('detects an exact repeated paragraph loop', () => {
    const paragraph =
      'The file does not exist in the workspace. Please verify the path.\n\n';
    const paddedParagraph = paragraph.padEnd(
      REPEATED_TEXT_MIN_PATTERN_LENGTH,
      '.',
    );
    const input = paddedParagraph.repeat(REPEATED_TEXT_MIN_REPETITIONS);

    const result = detectRepeatedTextLoop(input);

    expect(result).toMatchObject({
      patternLength: paddedParagraph.length,
      repetitionCount: REPEATED_TEXT_MIN_REPETITIONS,
    });
  });
});
