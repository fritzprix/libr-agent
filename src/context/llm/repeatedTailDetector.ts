export interface RepeatedTailMatch {
  observedTailChars: number;
  patternLength: number;
  repetitionCount: number;
}

export interface RepeatedTailDetectorConfig {
  minPatternLength: number;
  minRepetitions: number;
  tailChars: number;
}

function measureOverlapEndingAt(
  text: string,
  boundaryIndex: number,
  shift: number,
): number {
  let overlapLength = 0;
  let currentIndex = boundaryIndex;
  let shiftedIndex = boundaryIndex - shift;

  while (
    currentIndex >= 0 &&
    shiftedIndex >= 0 &&
    text[currentIndex] === text[shiftedIndex]
  ) {
    overlapLength += 1;
    currentIndex -= 1;
    shiftedIndex -= 1;
  }

  return overlapLength;
}

export function detectRepeatedTailLoop(
  text: string,
  config: RepeatedTailDetectorConfig,
): RepeatedTailMatch | null {
  const { minPatternLength, minRepetitions, tailChars } = config;

  if (text.length < minPatternLength * minRepetitions) {
    return null;
  }

  const tail = text.slice(-tailChars);
  const maxPatternLength = Math.floor(
    (tail.length - minPatternLength) / (minRepetitions - 1),
  );

  for (
    let patternLength = maxPatternLength;
    patternLength >= minPatternLength;
    patternLength--
  ) {
    let boundaryIndex = tail.length - 1;
    let repetitionCount = 1;

    while (boundaryIndex - patternLength >= 0) {
      const overlapLength = measureOverlapEndingAt(
        tail,
        boundaryIndex,
        patternLength,
      );
      if (overlapLength < minPatternLength) {
        break;
      }

      repetitionCount += 1;
      if (repetitionCount >= minRepetitions) {
        return {
          observedTailChars: tail.length,
          patternLength,
          repetitionCount,
        };
      }

      boundaryIndex -= patternLength;
    }
  }

  return null;
}

export const REPEATED_THINKING_TAIL_CHARS = 1024;
export const REPEATED_THINKING_MIN_PATTERN_LENGTH = 64;
export const REPEATED_THINKING_MIN_REPETITIONS = 2;

export const REPEATED_THINKING_CONFIG: RepeatedTailDetectorConfig = {
  minPatternLength: REPEATED_THINKING_MIN_PATTERN_LENGTH,
  minRepetitions: REPEATED_THINKING_MIN_REPETITIONS,
  tailChars: REPEATED_THINKING_TAIL_CHARS,
};

export function detectRepeatedThinkingLoop(
  thinking: string,
): RepeatedTailMatch | null {
  return detectRepeatedTailLoop(thinking, REPEATED_THINKING_CONFIG);
}

export const REPEATED_TEXT_TAIL_CHARS = 1024;
export const REPEATED_TEXT_MIN_PATTERN_LENGTH = 64;
export const REPEATED_TEXT_MIN_REPETITIONS = 3;

export const REPEATED_TEXT_CONFIG: RepeatedTailDetectorConfig = {
  minPatternLength: REPEATED_TEXT_MIN_PATTERN_LENGTH,
  minRepetitions: REPEATED_TEXT_MIN_REPETITIONS,
  tailChars: REPEATED_TEXT_TAIL_CHARS,
};

export function detectRepeatedTextLoop(text: string): RepeatedTailMatch | null {
  return detectRepeatedTailLoop(text, REPEATED_TEXT_CONFIG);
}
