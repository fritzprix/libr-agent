export const REPEATED_THINKING_TAIL_CHARS = 1024;
export const REPEATED_THINKING_MIN_PATTERN_LENGTH = 64;
export const REPEATED_THINKING_MIN_REPETITIONS = 2;

export interface RepeatedThinkingMatch {
  observedTailChars: number;
  patternLength: number;
  repetitionCount: number;
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

export function detectRepeatedThinkingLoop(
  thinking: string,
): RepeatedThinkingMatch | null {
  if (
    thinking.length <
    REPEATED_THINKING_MIN_PATTERN_LENGTH * REPEATED_THINKING_MIN_REPETITIONS
  ) {
    return null;
  }

  const tail = thinking.slice(-REPEATED_THINKING_TAIL_CHARS);
  const maxPatternLength = Math.floor(
    (tail.length - REPEATED_THINKING_MIN_PATTERN_LENGTH) /
      (REPEATED_THINKING_MIN_REPETITIONS - 1),
  );

  for (
    let patternLength = maxPatternLength;
    patternLength >= REPEATED_THINKING_MIN_PATTERN_LENGTH;
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
      if (overlapLength < REPEATED_THINKING_MIN_PATTERN_LENGTH) {
        break;
      }

      repetitionCount += 1;
      if (repetitionCount >= REPEATED_THINKING_MIN_REPETITIONS) {
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
