import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Card } from '@/components/ui';
import type { MigrationPhase } from '../useMigration';

export type MigrationLoadingPhase = Extract<
  MigrationPhase,
  'inspecting' | 'exporting' | 'importing' | 'selecting'
>;

export interface MigrationLoadingCardProps {
  phase: MigrationLoadingPhase;
  progress: number;
}

export const MigrationLoadingCard: FC<MigrationLoadingCardProps> = ({
  phase,
  progress,
}) => {
  const { t } = useTranslation('common');

  return (
    <Card className="border border-border/80 shadow-md bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center justify-center gap-6 text-center animate-in fade-in duration-300">
      <div className="relative flex items-center justify-center">
        <RefreshCw className="h-12 w-12 text-primary animate-spin" />
        <span className="absolute text-[10px] font-bold text-primary">
          {progress}%
        </span>
      </div>
      <div>
        <h3 className="text-lg font-bold text-foreground capitalize">
          {phase === 'inspecting' &&
            t(
              'settings.migration.loading.inspectingTitle',
              'Analyzing file structure...',
            )}
          {phase === 'exporting' &&
            t(
              'settings.migration.loading.exportingTitle',
              'Writing migration file...',
            )}
          {phase === 'importing' &&
            t(
              'settings.migration.loading.importingTitle',
              'Applying to environment database...',
            )}
          {phase === 'selecting' &&
            t(
              'settings.migration.loading.selectingTitle',
              'Waiting for user input...',
            )}
        </h3>
        <p className="text-sm text-muted-foreground mt-2 max-w-sm">
          {phase === 'inspecting' &&
            t(
              'settings.migration.loading.inspectingDescription',
              'Inspecting contents and performing threat analysis for ZIP Slip protection.',
            )}
          {phase === 'exporting' &&
            t(
              'settings.migration.loading.exportingDescription',
              'Serializing settings tables and archiving skill files.',
            )}
          {phase === 'importing' &&
            t(
              'settings.migration.loading.importingDescription',
              'Applying settings within an isolated transaction and validating foreign key constraints.',
            )}
          {phase === 'selecting' &&
            t(
              'settings.migration.loading.selectingDescription',
              'Complete your selection in the folder/file explorer window.',
            )}
        </p>
      </div>
      <div className="w-full max-w-xs bg-muted rounded-full h-1.5 overflow-hidden">
        <div
          className="bg-primary h-full transition-all duration-300"
          style={{ width: `${progress}%` }}
        ></div>
      </div>
    </Card>
  );
};

export function isLoadingPhase(
  phase: MigrationPhase,
): phase is MigrationLoadingPhase {
  return (
    phase === 'inspecting' ||
    phase === 'exporting' ||
    phase === 'importing' ||
    phase === 'selecting'
  );
}
