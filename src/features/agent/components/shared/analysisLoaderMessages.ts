export type CombinatorialParts = {
  /** Must include {{action}}, {{target}}, and {{flavor}} placeholders. */
  template: string;
  actions: string[];
  targets: string[];
  flavors: string[];
};

export const DEFAULT_INITIAL = 'Preparing response...';

export const DEFAULT_WITTY = [
  'Sipping digital coffee...',
  'Neurons are stretching...',
  'Picking the most intellectual emojis...',
  'Navigating through 0s and 1s...',
  'Tabs or spaces? Debating the eternal question...',
  'Tuning quantum entanglement...',
  'Hunting for missing semicolons...',
  'Dusting off virtual bookshelves...',
  'GPU fans spinning at full speed 🌪️',
  'Baking a fresh, crispy response 🥐',
  'Briefly admiring a cat picture 🐾',
  'Searching every corner of cache memory...',
];

export const DEFAULT_LATE = [
  'Great answers require proper aging, like fine wine 🍷',
  'No progress bar, so cycling witty text instead...',
  'Waiting 3 more seconds might unleash pure genius...',
  'Almost there, hang tight!',
  'Taking a moment? Time for a quick stretch 🧘',
  'Thanks for your patience, almost ready...',
];

export const DEFAULT_COMBINATORIAL: CombinatorialParts = {
  template: '{{action}} {{target}}{{flavor}}',
  actions: [
    'Sipping',
    'Tuning',
    'Baking',
    'Hunting for',
    'Dusting off',
    'Spinning up',
    'Warming',
    'Polishing',
    'Unwrapping',
    'Recalibrating',
    'Untangling',
    'Gently poking',
  ],
  targets: [
    'digital coffee',
    'quantum neurons',
    'crispy responses',
    'virtual bookshelves',
    'missing semicolons',
    'cat pictures',
    'GPU fans',
    'cache corners',
    'entangled tokens',
    'witty emojis',
    'latent thoughts',
    'half-baked ideas',
  ],
  flavors: [
    '...',
    ' with care...',
    ' at full speed...',
    ' one more time...',
    ' quietly...',
    ' 🌪️',
    ' 🥐',
    ' 🐾',
    ' ✨',
  ],
};

export function isStringArray(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.every((item) => typeof item === 'string')
  );
}

export function shuffleArray<T>(array: readonly T[]): T[] {
  const result = [...array];
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

export function fillTemplate(
  template: string,
  vars: Record<string, string>,
): string {
  return template
    .replace(/\{\{(\w+)\}\}/g, (_, key: string) => vars[key] ?? '')
    .replace(/\s+/g, ' ')
    .trim();
}

export function parseCombinatorialParts(
  raw: unknown,
): CombinatorialParts | null {
  if (!raw || typeof raw !== 'object') {
    return null;
  }

  const record = raw as Record<string, unknown>;
  const template =
    typeof record.template === 'string' ? record.template.trim() : '';
  if (
    !template.includes('{{action}}') ||
    !template.includes('{{target}}') ||
    !template.includes('{{flavor}}')
  ) {
    return null;
  }

  if (
    !isStringArray(record.actions) ||
    !isStringArray(record.targets) ||
    !isStringArray(record.flavors)
  ) {
    return null;
  }

  return {
    template,
    actions: record.actions,
    targets: record.targets,
    flavors: record.flavors,
  };
}

/** Sample unique combinatorial messages without materializing the full cartesian product. */
export function sampleCombinatorial(
  parts: CombinatorialParts,
  count: number,
  exclude: ReadonlySet<string> = new Set(),
): string[] {
  const { template, actions, targets, flavors } = parts;
  if (actions.length === 0 || targets.length === 0 || count <= 0) {
    return [];
  }

  const flavorList = flavors.length > 0 ? flavors : [''];
  const capacity = actions.length * targets.length * flavorList.length;
  const targetCount = Math.min(count, capacity);
  if (targetCount === 0) {
    return [];
  }

  const results: string[] = [];
  const seen = new Set(exclude);
  // Cap attempts and bail on long miss streaks so a near-full exclude set
  // cannot spin toward targetCount * 40 when almost nothing remains.
  const maxAttempts = targetCount * 40;
  let attempts = 0;
  let consecutiveMisses = 0;

  while (results.length < targetCount && attempts < maxAttempts) {
    attempts += 1;
    const message = fillTemplate(template, {
      action: actions[Math.floor(Math.random() * actions.length)] ?? '',
      target: targets[Math.floor(Math.random() * targets.length)] ?? '',
      flavor: flavorList[Math.floor(Math.random() * flavorList.length)] ?? '',
    });
    if (message.length === 0 || seen.has(message)) {
      consecutiveMisses += 1;
      // Coupon-collector near exhaustion: allow several full passes before giving up.
      if (consecutiveMisses >= Math.max(capacity * 8, 30)) {
        break;
      }
      continue;
    }
    consecutiveMisses = 0;
    seen.add(message);
    results.push(message);
  }

  return results;
}

export function buildEarlyPool(
  initial: string,
  witty: readonly string[],
  parts: CombinatorialParts,
  combinatorialSampleSize = 16,
): string[] {
  const initialMessage = initial.trim().length > 0 ? initial : DEFAULT_INITIAL;
  const curatedRest = witty.filter(
    (item) => item.length > 0 && item !== initialMessage,
  );
  const combinatorial = sampleCombinatorial(
    parts,
    combinatorialSampleSize,
    new Set([initialMessage, ...curatedRest]),
  );
  // Keep initial pinned for the first eye-landing beat; shuffle only the rest.
  return [initialMessage, ...shuffleArray([...curatedRest, ...combinatorial])];
}

export function buildEndlessPool(
  witty: readonly string[],
  parts: CombinatorialParts,
  combinatorialSampleSize = 24,
  avoidFirst?: string,
): string[] {
  const wittyList = witty.filter((item) => item.length > 0);
  const combinatorial = sampleCombinatorial(
    parts,
    combinatorialSampleSize,
    new Set(wittyList),
  );
  const pool = shuffleArray([...wittyList, ...combinatorial]);
  if (pool.length === 0) {
    return wittyList.length > 0 ? [...wittyList] : [DEFAULT_INITIAL];
  }
  if (avoidFirst && pool[0] === avoidFirst && pool.length > 1) {
    const [first, ...rest] = pool;
    return [...rest, first];
  }
  return pool;
}

export type LoaderPhase = 'early' | 'late' | 'endless';

export function nextPhase(phase: LoaderPhase): LoaderPhase {
  if (phase === 'early') {
    return 'late';
  }
  if (phase === 'late') {
    return 'endless';
  }
  return 'endless';
}
