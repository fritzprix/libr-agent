export const TOKEN_UNIT = 1024;
export const SLIDER_MAX_POSITION = 1000;

// Keyboard fine-tuning steps. The slider track is logarithmic, so one position
// step can round back to the same value near the low end. Instead, arrow keys
// move the token value itself by exactly one 1K step (PageUp/PageDown or
// Shift+Arrow by ten), so every 1K value in the range is reachable.
export const KEYBOARD_TOKEN_STEP = TOKEN_UNIT;
export const KEYBOARD_TOKEN_FAST_STEP = TOKEN_UNIT * 10;

export function clampAndRoundTokenValue(
  value: number,
  min: number,
  max: number,
): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  const rounded = Math.round(value / TOKEN_UNIT) * TOKEN_UNIT;
  return Math.min(max, Math.max(min, rounded));
}

export function valueToSliderPosition(
  value: number,
  min: number,
  max: number,
): number {
  if (max <= min) {
    return 0;
  }
  const clamped = Math.min(max, Math.max(min, value));
  const ratio =
    (Math.log(clamped) - Math.log(min)) / (Math.log(max) - Math.log(min));
  return Math.round(ratio * SLIDER_MAX_POSITION);
}

export function sliderPositionToValue(
  position: number,
  min: number,
  max: number,
): number {
  if (position <= 0) {
    return min;
  }
  if (position >= SLIDER_MAX_POSITION) {
    return max;
  }
  const raw =
    min * Math.exp((Math.log(max / min) * position) / SLIDER_MAX_POSITION);
  return clampAndRoundTokenValue(raw, min, max);
}

export function formatTokenValue(value: number): string {
  // The label must identify the exact 1K value: 1049600 (1025K) shows as
  // "1025K", while exact megabyte values keep the short "1M"/"4M" form.
  if (value > 0 && value % (TOKEN_UNIT * TOKEN_UNIT) === 0) {
    return `${value / (TOKEN_UNIT * TOKEN_UNIT)}M`;
  }
  if (value >= TOKEN_UNIT && value % TOKEN_UNIT === 0) {
    return `${value / TOKEN_UNIT}K`;
  }
  return String(value);
}
