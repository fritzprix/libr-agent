import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
} from '@/components/ui';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import LoadingSpinner from '@/components/ui/LoadingSpinner';

interface DangerZoneSettingsProps {
  isDeleting: boolean;
  isResetting: boolean;
  onDelete: () => Promise<void>;
  onReset: () => Promise<void>;
}

export function DangerZoneSettings({
  isDeleting,
  isResetting,
  onDelete,
  onReset,
}: DangerZoneSettingsProps) {
  const { t } = useTranslation('common');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [resetConfirmOpen, setResetConfirmOpen] = useState(false);
  const [resetConfirmationText, setResetConfirmationText] = useState('');

  const isFactoryResetConfirmed = resetConfirmationText.trim() === 'RESET';

  return (
    <div className="mt-4 rounded-xl border border-destructive/25 bg-destructive/5 p-6">
      <h3 className="text-lg font-medium text-destructive mb-4 flex items-center gap-2">
        {t('settings.dangerZone.title', '⚠️ Danger Zone')}
      </h3>
      <p className="mb-6 text-sm text-muted-foreground">
        {t(
          'settings.dangerZone.description',
          'These actions permanently delete local data. Double-check the scope before continuing.',
        )}
      </p>
      <div className="space-y-6">
        {/* Delete All Sessions Card */}
        <Card className="border border-destructive/25 bg-background shadow-sm">
          <CardHeader className="pb-4">
            <CardTitle className="text-foreground text-base font-medium">
              {t('settings.dataReset.title', 'Data & Reset')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.dataReset.description',
                'This will permanently delete all local sessions, their messages, and workspace file stores.',
              )}
            </p>
            <div className="flex items-center justify-start pt-4 gap-x-2">
              <Button
                type="button"
                variant="destructive"
                disabled={isDeleting}
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setConfirmOpen(true);
                }}
              >
                {isDeleting && <LoadingSpinner size="sm" className="mr-2" />}
                <span>
                  {isDeleting
                    ? t('settings.dataReset.deleting', 'Deleting...')
                    : t(
                        'settings.dataReset.clearAll',
                        'Clear All Sessions & Workspace',
                      )}
                </span>
              </Button>
              <AlertDialog
                open={confirmOpen}
                onOpenChange={(open) =>
                  !open && !isDeleting && setConfirmOpen(false)
                }
              >
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>
                      {t(
                        'settings.dataReset.confirmTitle',
                        'Delete All Sessions, Messages & Workspace',
                      )}
                    </AlertDialogTitle>
                    <AlertDialogDescription>
                      {t(
                        'settings.dataReset.confirmDescription',
                        'This will permanently delete all local sessions, their messages, and workspace file stores. This action cannot be undone. Are you sure you want to continue?',
                      )}
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel disabled={isDeleting}>
                      {t('common.cancel', 'Cancel')}
                    </AlertDialogCancel>
                    <AlertDialogAction
                      onClick={(e) => {
                        e.preventDefault();
                        void onDelete().then(() => setConfirmOpen(false));
                      }}
                      disabled={isDeleting}
                    >
                      {isDeleting && (
                        <LoadingSpinner size="sm" className="mr-2" />
                      )}
                      {t('common.delete', 'Delete')}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          </CardContent>
        </Card>

        {/* Factory Reset Card */}
        <Card className="border border-destructive/25 bg-background shadow-sm">
          <CardHeader className="pb-4">
            <CardTitle className="text-foreground text-base font-medium">
              {t('settings.factoryReset.title', 'Factory Reset')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.factoryReset.description',
                'This will perform a complete factory reset. It deletes ALL data.',
              )}
            </p>
            <div className="flex items-center justify-start pt-4 gap-x-2">
              <Button
                type="button"
                variant="destructive"
                disabled={isResetting || isDeleting}
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setResetConfirmOpen(true);
                }}
              >
                {isResetting && <LoadingSpinner size="sm" className="mr-2" />}
                <span>
                  {isResetting
                    ? t('settings.factoryReset.resetting', 'Resetting...')
                    : t(
                        'settings.factoryReset.button',
                        'Reset All Data & Settings',
                      )}
                </span>
              </Button>
              <AlertDialog
                open={resetConfirmOpen}
                onOpenChange={(open) => {
                  if (open) {
                    setResetConfirmOpen(true);
                    return;
                  }

                  if (isResetting) {
                    return;
                  }

                  setResetConfirmOpen(false);
                  setResetConfirmationText('');
                }}
              >
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>
                      {t(
                        'settings.factoryReset.confirmTitle',
                        'Factory Reset Confirmation',
                      )}
                    </AlertDialogTitle>
                    <AlertDialogDescription>
                      {t(
                        'settings.factoryReset.confirmDescription',
                        'This will permanently delete ALL data including sessions, assistants, MCP servers, and playbooks. Are you sure?',
                      )}
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <div className="space-y-2">
                    <p className="text-sm font-medium text-destructive">
                      {t(
                        'settings.factoryReset.typeReset',
                        'Type RESET to enable factory reset.',
                      )}
                    </p>
                    <Input
                      value={resetConfirmationText}
                      onChange={(event) =>
                        setResetConfirmationText(event.target.value)
                      }
                      placeholder={t(
                        'settings.factoryReset.typeResetPlaceholder',
                        'Type RESET',
                      )}
                      className="font-mono"
                      autoFocus
                      disabled={isResetting}
                    />
                  </div>
                  <AlertDialogFooter>
                    <AlertDialogCancel disabled={isResetting}>
                      {t('common.cancel', 'Cancel')}
                    </AlertDialogCancel>
                    <AlertDialogAction
                      onClick={(e) => {
                        e.preventDefault();
                        void onReset().then(() => {
                          setResetConfirmOpen(false);
                          setResetConfirmationText('');
                        });
                      }}
                      disabled={isResetting || !isFactoryResetConfirmed}
                      className="bg-destructive text-white hover:bg-destructive/90"
                    >
                      {isResetting && (
                        <LoadingSpinner size="sm" className="mr-2" />
                      )}
                      {t(
                        'settings.factoryReset.confirmButton',
                        'Reset Everything',
                      )}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
