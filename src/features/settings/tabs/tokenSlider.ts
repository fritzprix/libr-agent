export const TOKEN_UNIT = 1024;
export const SLIDER_MAX_POSITION = 1000;

export function clampAndRoundTokenValue(
  value: number,
  min: number,
  max: number,
  unit: number = TOKEN_UNIT,
): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  const rounded = Math.round(value / unit) * unit;
  return Math.min(max, Math.max(min, rounded));
}

export function valueToSliderPosition(
  value: number,
  min: number,
  max: number,
  steps: number = SLIDER_MAX_POSITION,
): number {
  if (max <= min) {
    return 0;
  }
  const clamped = Math.min(max, Math.max(min, value));
  const ratio =
    (Math.log(clamped) - Math.log(min)) / (Math.log(max) - Math.log(min));
  return Math.round(ratio * steps);
}

export function sliderPositionToValue(
  position: number,
  min: number,
  max: number,
  steps: number = SLIDER_MAX_POSITION,
  unit: number = TOKEN_UNIT,
): number {
  if (position <= 0) {
    return min;
  }
  if (position >= steps) {
    return max;
  }
  const raw = min * Math.exp((Math.log(max / min) * position) / steps);
  return clampAndRoundTokenValue(raw, min, max, unit);
}

export function formatTokenValue(value: number): string {
  if (value >= 1048576) {
    // 1M, 2M, 4M; one decimal for a stored value in between, such as 1.5M.
    return `${Math.round((value / 1048576) * 10) / 10}M`;
  }
  return value >= TOKEN_UNIT ? `${Math.round(value / TOKEN_UNIT)}K` : `${value}`;
}
