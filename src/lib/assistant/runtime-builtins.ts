import {
  BUILTIN_SERVICE_CANONICAL_NAMES,
  BuiltinServiceCanonicalName,
  CORE_BUILTIN_SERVICE_ALIASES,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '../generated/builtin-services';

// Re-export the constants so existing imports still work
export { CORE_BUILTIN_SERVICE_ALIASES, OPTIONAL_BUILTIN_SERVICE_ALIASES };

/** Known canonical names. O(1) lookup. */
const _canonicals = new Set<string>(BUILTIN_SERVICE_CANONICAL_NAMES);

/** Runtime type guard: checks if a string is a known canonical service name. */
function isBuiltinServiceId(s: string): s is BuiltinServiceCanonicalName {
  return _canonicals.has(s);
}

function canonicalizeAlias(alias: string): BuiltinServiceCanonicalName | null {
  const normalized = alias.trim().toLowerCase();
  return isBuiltinServiceId(normalized) ? normalized : null;
}

// ─── Runtime enforcement ───────────────────────────────────────────────────────

export function enforceRuntimeBuiltinAliases(
  aliases: string[] | undefined,
): string[] {
  const ordered = BUILTIN_SERVICE_CANONICAL_NAMES;
  const allowed = new Set<string>(CORE_BUILTIN_SERVICE_ALIASES);

  if (aliases !== undefined) {
    // Explicit list: add only the canonicalized aliases (may be empty → core only)
    for (const alias of aliases) {
      const canonical = canonicalizeAlias(alias);
      if (canonical) allowed.add(canonical);
    }
  } else {
    // No list at all → all optional services are implicitly enabled
    for (const alias of OPTIONAL_BUILTIN_SERVICE_ALIASES) {
      allowed.add(alias);
    }
  }

  return ordered.filter((alias) => allowed.has(alias));
}
