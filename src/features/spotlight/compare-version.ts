/**
 * Compare dotted version strings (e.g. "0.9.21").
 * Returns negative if a < b, 0 if equal, positive if a > b.
 * Non-numeric segments compare as 0.
 *
 * Limitation: prerelease / build metadata (e.g. "0.9.21-beta.1") is not SemVer-aware;
 * the non-numeric part becomes 0, so "0.9.21-beta" can compare equal to "0.9.21".
 * Fine while `__APP_VERSION__` stays plain dotted versions.
 */
export function compareVersion(a: string, b: string): number {
  const parse = (value: string): number[] =>
    value
      .trim()
      .replace(/^v/i, '')
      .split('.')
      .map((part) => {
        const n = Number.parseInt(part, 10);
        return Number.isFinite(n) ? n : 0;
      });

  const left = parse(a);
  const right = parse(b);
  const len = Math.max(left.length, right.length);

  for (let i = 0; i < len; i += 1) {
    const l = left[i] ?? 0;
    const r = right[i] ?? 0;
    if (l !== r) {
      return l - r;
    }
  }
  return 0;
}

export function isVersionGreater(a: string, b: string): boolean {
  return compareVersion(a, b) > 0;
}
