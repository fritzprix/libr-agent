export type RuntimeHintKind = 'node' | 'uv' | 'docker' | null;

export interface HumanizedVerificationError {
  /** Short user-facing explanation */
  summary: string;
  /** Optional follow-up guidance */
  guidance?: string;
  /** Which runtime installer to point at */
  runtime: RuntimeHintKind;
  /** Raw backend message (kept for power users / title tooltip) */
  raw: string;
}

function includesAny(haystack: string, needles: readonly string[]): boolean {
  return needles.some((needle) => haystack.includes(needle));
}

/**
 * Map low-level probe failures into actionable copy (especially missing npx/uvx).
 */
export function humanizeVerificationError(
  rawError: string | undefined | null,
): HumanizedVerificationError | null {
  if (!rawError || !rawError.trim()) {
    return null;
  }

  const raw = rawError.trim();
  const normalized = raw.toLowerCase();

  const missingBinary =
    includesAny(normalized, [
      'no such file or directory',
      'os error 2',
      'not found',
      'enoent',
      'cannot find',
      'is not recognized',
      'command not found',
    ]) || normalized.includes('program not found');

  if (missingBinary && includesAny(normalized, ['npx', 'npm', 'node'])) {
    return {
      summary: 'Node.js / npx was not found on this machine.',
      guidance:
        'Install Node.js via Chat → App Wizard (or mention @skill:setup-wizard), then tap Retry.',
      runtime: 'node',
      raw,
    };
  }

  if (missingBinary && includesAny(normalized, ['uvx', 'uv'])) {
    return {
      summary: 'uv / uvx was not found on this machine.',
      guidance:
        'Install uv via Chat → App Wizard (or mention @skill:setup-wizard), then tap Retry.',
      runtime: 'uv',
      raw,
    };
  }

  if (missingBinary && includesAny(normalized, ['docker'])) {
    return {
      summary: 'Docker was not found or is not running.',
      guidance:
        'Install or start Docker, then tap Retry. App Wizard can also help diagnose the environment.',
      runtime: 'docker',
      raw,
    };
  }

  if (missingBinary) {
    return {
      summary: 'A required command was not found on this machine.',
      guidance:
        'Most recommended extensions need Node.js (npx) or uv (uvx). Open App Wizard to install runtimes, then tap Retry.',
      runtime: 'node',
      raw,
    };
  }

  if (normalized.includes('timed out') || normalized.includes('timeout')) {
    return {
      summary: 'Connection timed out while starting the extension.',
      guidance:
        'First installs may download packages for a minute or more. Check your network, then tap Retry.',
      runtime: null,
      raw,
    };
  }

  return {
    summary: raw.length > 180 ? `${raw.slice(0, 177)}…` : raw,
    runtime: null,
    raw,
  };
}

/**
 * Progressive copy while verification is pending (no stdout stream yet).
 */
export function pendingVerificationHint(elapsedSeconds: number): string {
  if (elapsedSeconds < 8) {
    return 'Starting extension process…';
  }
  if (elapsedSeconds < 30) {
    return 'Downloading or starting packages (first install can take a bit)…';
  }
  if (elapsedSeconds < 90) {
    return 'Still working — large downloads or git builds can take up to a couple of minutes…';
  }
  return 'Taking longer than usual. You can keep working; status will update when ready.';
}
