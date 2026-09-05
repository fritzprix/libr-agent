import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui';
import { Input } from '@/components/ui/input';

export interface MigrationPasswordDialogsProps {
  isExportPasswordOpen: boolean;
  setIsExportPasswordOpen: (open: boolean) => void;
  exportPassword: string;
  setExportPassword: (value: string) => void;
  exportPasswordConfirm: string;
  setExportPasswordConfirm: (value: string) => void;
  exportPasswordError: string | null;
  onExportWithPassword: () => Promise<void>;
  isImportPasswordOpen: boolean;
  importPassword: string;
  setImportPassword: (value: string) => void;
  importPasswordError: string | null;
  onCancelImportPassword: () => void;
  onImportPasswordSubmit: () => Promise<void>;
}

export const MigrationPasswordDialogs: FC<MigrationPasswordDialogsProps> = ({
  isExportPasswordOpen,
  setIsExportPasswordOpen,
  exportPassword,
  setExportPassword,
  exportPasswordConfirm,
  setExportPasswordConfirm,
  exportPasswordError,
  onExportWithPassword,
  isImportPasswordOpen,
  importPassword,
  setImportPassword,
  importPasswordError,
  onCancelImportPassword,
  onImportPasswordSubmit,
}) => {
  const { t } = useTranslation('common');

  return (
    <>
      <Dialog
        open={isExportPasswordOpen}
        onOpenChange={setIsExportPasswordOpen}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              {t(
                'settings.migration.exportPassword.title',
                'Set backup security password',
              )}
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              {t(
                'settings.migration.exportPassword.description',
                'Because this backup includes sensitive data (API keys, credentials, etc.), you must set a password to encrypt the backup file. This password is required when restoring (importing) the backup later.',
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.exportPassword.passwordLabel',
                  'Password (minimum 4 characters)',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.exportPassword.passwordPlaceholder',
                  'Enter password',
                )}
                value={exportPassword}
                onChange={(e) => setExportPassword(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.exportPassword.confirmLabel',
                  'Confirm password',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.exportPassword.confirmPlaceholder',
                  'Re-enter password',
                )}
                value={exportPasswordConfirm}
                onChange={(e) => setExportPasswordConfirm(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {exportPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {exportPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setIsExportPasswordOpen(false)}
              className="rounded-lg h-9"
            >
              {t('settings.migration.exportPassword.cancel', 'Cancel')}
            </Button>
            <Button
              size="sm"
              onClick={onExportWithPassword}
              className="rounded-lg h-9 font-semibold"
            >
              {t('settings.migration.exportPassword.runExport', 'Run Export')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isImportPasswordOpen}
        onOpenChange={(open) => {
          if (!open) onCancelImportPassword();
        }}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              {t(
                'settings.migration.importPassword.title',
                'Enter security password',
              )}
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              {t(
                'settings.migration.importPassword.description',
                'This backup file is encrypted. A password is required to preview and import it. Enter the password you set when exporting.',
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                {t(
                  'settings.migration.importPassword.passwordLabel',
                  'Password',
                )}
              </label>
              <Input
                type="password"
                placeholder={t(
                  'settings.migration.importPassword.passwordPlaceholder',
                  'Enter password',
                )}
                value={importPassword}
                onChange={(e) => setImportPassword(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') onImportPasswordSubmit();
                }}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {importPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {importPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={onCancelImportPassword}
              className="rounded-lg h-9"
            >
              {t('settings.migration.importPassword.cancel', 'Cancel')}
            </Button>
            <Button
              size="sm"
              onClick={onImportPasswordSubmit}
              className="rounded-lg h-9 font-semibold"
            >
              {t('settings.migration.importPassword.confirm', 'Confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
};
