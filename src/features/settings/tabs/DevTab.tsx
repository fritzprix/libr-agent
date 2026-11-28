import { useTranslation } from 'react-i18next';

/**
 * Dev-only settings tab (#3093).
 *
 * The `sampleText` / `compact` tester was removed: it called the TypeScript
 * AI services directly, so it tested a path users never run. The Rust path is
 * tested from the session UI instead. This shell stays for future dev-only
 * tooling.
 */
export default function DevTab() {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6 p-1">
      <div className="flex items-center gap-2">
        <span className="rounded bg-yellow-500/20 px-2 py-0.5 text-xs font-mono text-yellow-500">
          {t('settings.dev.devOnly', 'DEV ONLY')}
        </span>
      </div>
    </div>
  );
}
