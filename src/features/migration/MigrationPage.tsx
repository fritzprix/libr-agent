import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, ShieldAlert } from 'lucide-react';
import { Button } from '@/components/ui';
import { useMigrationPage } from './hooks/useMigrationPage';
import { MigrationIdleCards } from './components/MigrationIdleCards';
import {
  isLoadingPhase,
  MigrationLoadingCard,
} from './components/MigrationLoadingCard';
import { MigrationPreviewCard } from './components/MigrationPreviewCard';
import { MigrationCompleteCard } from './components/MigrationCompleteCard';
import { MigrationPasswordDialogs } from './components/MigrationPasswordDialogs';

const MigrationPage: FC = () => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const {
    phase,
    preview,
    exportInfo,
    importResult,
    error,
    progress,
    selectedExportDir,
    includeSensitiveData,
    setIncludeSensitiveData,
    selectExportFile,
    selectImportFile,
    reset,
    strategy,
    setStrategy,
    reverifying,
    isExportPasswordOpen,
    setIsExportPasswordOpen,
    exportPassword,
    setExportPassword,
    exportPasswordConfirm,
    setExportPasswordConfirm,
    exportPasswordError,
    isImportPasswordOpen,
    importPassword,
    setImportPassword,
    importPasswordError,
    handleCancelImportPassword,
    handleImportPasswordSubmit,
    handleExport,
    handleExportWithPassword,
    handleImport,
    handleReverify,
  } = useMigrationPage();

  const showIdleCards =
    phase === 'idle' && !preview && !exportInfo && !importResult;
  const showPreview = phase === 'idle' && preview !== null && !importResult;
  const showComplete =
    phase === 'complete' && (exportInfo !== null || importResult !== null);

  return (
    <div className="p-6 min-h-full bg-background/50 flex flex-col items-center">
      <div className="max-w-3xl w-full flex flex-col gap-6">
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => navigate('/settings')}
            className="h-10 w-10 rounded-xl"
          >
            <ArrowLeft className="h-5 w-5 text-muted-foreground" />
          </Button>
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              {t('settings.migration.title', 'Data Migration')}
            </h1>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.migration.subtitle',
                'Transfer LibrAgent settings, assistants, scheduler, and custom skills to another device.',
              )}
            </p>
          </div>
        </div>

        {error &&
          error !== 'PASSWORD_REQUIRED' &&
          error !== 'WRONG_PASSWORD' && (
            <div className="p-4 rounded-xl border border-destructive/20 bg-destructive/10 text-destructive-foreground flex gap-3 items-start animate-in fade-in slide-in-from-top-4 duration-300">
              <ShieldAlert className="h-5 w-5 shrink-0 text-destructive mt-0.5" />
              <div className="text-sm flex-1">
                <span className="font-semibold">
                  {t('settings.migration.errors.occurred', 'Error occurred:')}
                </span>{' '}
                {error}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={reset}
                className="h-7 text-xs hover:bg-destructive/20 text-destructive-foreground"
              >
                {t('settings.migration.errors.reset', 'Reset')}
              </Button>
            </div>
          )}

        {showIdleCards && (
          <MigrationIdleCards
            includeSensitiveData={includeSensitiveData}
            setIncludeSensitiveData={setIncludeSensitiveData}
            selectedExportDir={selectedExportDir}
            selectExportFile={selectExportFile}
            selectImportFile={selectImportFile}
            onExport={handleExport}
          />
        )}

        {isLoadingPhase(phase) && (
          <MigrationLoadingCard phase={phase} progress={progress} />
        )}

        {showPreview && preview && (
          <MigrationPreviewCard
            preview={preview}
            strategy={strategy}
            setStrategy={setStrategy}
            onImport={handleImport}
            onReset={reset}
          />
        )}

        {showComplete && (
          <MigrationCompleteCard
            exportInfo={exportInfo}
            importResult={importResult}
            reverifying={reverifying}
            onReverify={handleReverify}
            onReset={reset}
          />
        )}
      </div>

      <MigrationPasswordDialogs
        isExportPasswordOpen={isExportPasswordOpen}
        setIsExportPasswordOpen={setIsExportPasswordOpen}
        exportPassword={exportPassword}
        setExportPassword={setExportPassword}
        exportPasswordConfirm={exportPasswordConfirm}
        setExportPasswordConfirm={setExportPasswordConfirm}
        exportPasswordError={exportPasswordError}
        onExportWithPassword={handleExportWithPassword}
        isImportPasswordOpen={isImportPasswordOpen}
        importPassword={importPassword}
        setImportPassword={setImportPassword}
        importPasswordError={importPasswordError}
        onCancelImportPassword={handleCancelImportPassword}
        onImportPasswordSubmit={handleImportPasswordSubmit}
      />
    </div>
  );
};

export default MigrationPage;
