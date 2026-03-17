// ─── Single source of truth ───────────────────────────────────────────────────
// To add, rename, or change a service: edit ONE row here.
// CORE_BUILTIN_SERVICE_ALIASES, OPTIONAL_BUILTIN_SERVICE_ALIASES, and
// canonicalizeAlias() are all derived from this registry automatically.

type ServiceCategory = 'core' | 'optional';

/**
 * Stable identifier for a builtin service.
 *
 * These are the canonical strings stored in `allowedBuiltInServiceAliases` for
 * new records.  The Rust side serialises `BuiltinServiceId` enum to these same
 * `snake_case` strings, so the set must stay in sync with
 * `src-tauri/src/mcp/builtin/service_id.rs`.
 *
 * Adding a new service: update this type AND add a row to `BUILTIN_SERVICE_REGISTRY`.
 */
export type BuiltinServiceId =
  | 'planning'
  | 'workspace'
  | 'knowledge'
  | 'assistant'
  | 'skills'
  | 'playbook'
  | 'attachments'
  | 'swarm'
  | 'ui'
  | 'browser'
  | 'bootstrap'
  | 'mcp_manager'
  | 'media';

interface BuiltinServiceEntry {
  readonly canonical: BuiltinServiceId;
  readonly category: ServiceCategory;
}

const BUILTIN_SERVICE_REGISTRY: readonly BuiltinServiceEntry[] = [
  { canonical: 'planning', category: 'core' },
  { canonical: 'workspace', category: 'core' },
  { canonical: 'knowledge', category: 'core' },
  { canonical: 'assistant', category: 'core' },
  { canonical: 'skills', category: 'core' },
  { canonical: 'playbook', category: 'core' },
  { canonical: 'attachments', category: 'core' },
  { canonical: 'swarm', category: 'core' },
  { canonical: 'ui', category: 'core' },
  { canonical: 'browser', category: 'optional' },
  { canonical: 'bootstrap', category: 'optional' },
  { canonical: 'mcp_manager', category: 'core' },
  { canonical: 'media', category: 'optional' },
];

// ─── Derived constants (do not edit directly) ──────────────────────────────────

export const CORE_BUILTIN_SERVICE_ALIASES: readonly string[] =
  BUILTIN_SERVICE_REGISTRY.filter((e) => e.category === 'core').map(
    (e) => e.canonical,
  );

export const OPTIONAL_BUILTIN_SERVICE_ALIASES: readonly string[] =
  BUILTIN_SERVICE_REGISTRY.filter((e) => e.category === 'optional').map(
    (e) => e.canonical,
  );

/** Known canonical names. O(1) lookup. */
const _canonicals = new Set(BUILTIN_SERVICE_REGISTRY.map((e) => e.canonical));

/** Runtime type guard: checks if a string is a known BuiltinServiceId. */
function isBuiltinServiceId(s: string): s is BuiltinServiceId {
  // The Set<BuiltinServiceId> .has() requires T; cast is safe because
  // we're INSIDE the type predicate — this IS the runtime validation.
  return (_canonicals as Set<string>).has(s);
}

// 🚨 DO NOT DELETE: Backward compatibility for pre-0.6.0 sessions
// Maps legacy service names stored in the database to their new canonical names.
// This ensures old sessions don't break when loading tools.
const LEGACY_SERVICE_MAPPINGS: Record<string, string> = {
  content_store: 'attachments',
};

function canonicalizeAlias(alias: string): BuiltinServiceId | null {
  let normalized = alias.trim().toLowerCase();

  // Apply legacy mappings if present
  if (LEGACY_SERVICE_MAPPINGS[normalized]) {
    normalized = LEGACY_SERVICE_MAPPINGS[normalized];
  }

  return isBuiltinServiceId(normalized) ? normalized : null;
}

// ─── Runtime enforcement ───────────────────────────────────────────────────────

export function enforceRuntimeBuiltinAliases(
  aliases: string[] | undefined,
): string[] {
  const ordered = BUILTIN_SERVICE_REGISTRY.map((e) => e.canonical);
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
