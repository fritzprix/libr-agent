import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowUpFromLine,
  ArrowDownToLine,
  FolderOpen,
  FileCheck2,
  Info,
  AlertTriangle,
} from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui';

export interface MigrationIdleCardsProps {
  includeSensitiveData: boolean;
  setIncludeSensitiveData: (val: boolean) => void;
  selectedExportDir: string | null;
  selectExportFile: () => Promise<void>;
  selectImportFile: () => Promise<void>;
  onExport: () => Promise<void>;
}

export const MigrationIdleCards: FC<MigrationIdleCardsProps> = ({
  includeSensitiveData,
  setIncludeSensitiveData,
  selectedExportDir,
  selectExportFile,
  selectImportFile,
  onExport,
}) => {
  const { t } = useTranslation('common');

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
      <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
        <CardHeader className="gap-2">
          <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <ArrowUpFromLine className="h-6 w-6" />
          </div>
          <CardTitle className="text-lg">
            {t('settings.migration.export.title', 'Export Settings')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.migration.export.description',
              'Package all settings and user skill data from this device into a single archive file (`.libragent-migration`).',
            )}
          </CardDescription>
        </CardHeader>

        <CardContent className="flex-1 flex flex-col gap-4">
          <div className="rounded-xl border border-warning/20 bg-warning/5 p-4 flex gap-3 items-start">
            <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
            <div className="text-xs text-muted-foreground leading-relaxed">
              <span className="font-semibold text-foreground">
                {t(
                  'settings.migration.export.securityWarningLabel',
                  '⚠️ Security notice:',
                )}
              </span>{' '}
              {t(
                'settings.migration.export.securityWarning',
                'API keys and access tokens are masked by default. Enable the option below only when moving them through another secure channel.',
              )}
            </div>
          </div>

          <div
            className="flex items-center gap-3 p-3 rounded-xl border bg-background/50 hover:bg-background/80 transition-colors duration-200 cursor-pointer"
            onClick={() => setIncludeSensitiveData(!includeSensitiveData)}
          >
            <input
              type="checkbox"
              checked={includeSensitiveData}
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setIncludeSensitiveData(e.target.checked)}
              className="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary cursor-pointer"
            />
            <div className="flex flex-col">
              <span className="text-xs font-semibold text-foreground">
                {t(
                  'settings.migration.export.includeSensitiveLabel',
                  'Include sensitive data (full Settings table)',
                )}
              </span>
              <span className="text-[10px] text-muted-foreground">
                {t(
                  'settings.migration.export.includeSensitiveDescription',
                  'Store plaintext data from the entire settings table—including API keys and access tokens—in the backup archive.',
                )}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <span className="text-xs font-semibold text-muted-foreground">
              {t(
                'settings.migration.export.selectPathLabel',
                'Select export path',
              )}
            </span>
            <div className="flex gap-2">
              <input
                type="text"
                readOnly
                placeholder={t(
                  'settings.migration.export.selectPathPlaceholder',
                  'Select a folder to export to...',
                )}
                value={selectedExportDir || ''}
                className="flex-1 px-3 py-2 text-xs rounded-lg border bg-background/30 focus:outline-none truncate"
              />
              <Button
                variant="outline"
                size="sm"
                onClick={selectExportFile}
                className="gap-1.5 h-9 rounded-lg"
              >
                <FolderOpen className="h-4 w-4" />
                {t('settings.migration.export.browse', 'Browse')}
              </Button>
            </div>
          </div>
        </CardContent>

        <CardFooter className="pt-2">
          <Button
            onClick={onExport}
            disabled={!selectedExportDir}
            className="w-full gap-2 rounded-xl h-10 shadow-sm"
          >
            <ArrowUpFromLine className="h-4 w-4" />
            {t('settings.migration.export.runExport', 'Run Export')}
          </Button>
        </CardFooter>
      </Card>

      <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
        <CardHeader className="gap-2">
          <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <ArrowDownToLine className="h-6 w-6" />
          </div>
          <CardTitle className="text-lg">
            {t('settings.migration.import.title', 'Import Settings')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.migration.import.description',
              'Load a previously exported `.libragent-migration` archive file to update the current environment.',
            )}
          </CardDescription>
        </CardHeader>

        <CardContent className="flex-1 flex flex-col justify-between">
          <div className="rounded-xl border border-primary/20 bg-primary/5 p-4 flex gap-3 items-start mb-6">
            <Info className="h-5 w-5 shrink-0 text-primary mt-0.5" />
            <div className="text-xs text-muted-foreground leading-relaxed">
              {t(
                'settings.migration.import.autoBackupInfo',
                'Before importing, an automatic backup of the current database is created to prevent data loss. If import fails, the previous state is restored immediately.',
              )}
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <span className="text-xs font-semibold text-muted-foreground">
              {t(
                'settings.migration.import.archiveFileLabel',
                'Migration archive file',
              )}
            </span>
            <Button
              variant="outline"
              onClick={selectImportFile}
              className="w-full gap-2 h-14 border-dashed border-2 hover:border-primary/50 hover:bg-primary/5 rounded-xl transition-all duration-200"
            >
              <FileCheck2 className="h-5 w-5 text-muted-foreground" />
              {t(
                'settings.migration.import.selectAndValidate',
                'Select file and validate',
              )}
            </Button>
          </div>
        </CardContent>

        <CardFooter className="pt-2">
          <Button disabled className="w-full gap-2 rounded-xl h-10">
            <ArrowDownToLine className="h-4 w-4" />
            {t(
              'settings.migration.import.runImportWaiting',
              'Run Import (awaiting file analysis)',
            )}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};
