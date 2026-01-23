import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
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
  onReset: () => void;
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

  return (
    <div className="border-t pt-8 mt-4">
      <h3 className="text-lg font-medium text-destructive mb-4 flex items-center gap-2">
        ⚠️ Danger Zone
      </h3>
      <div className="space-y-6">
        {/* Delete All Sessions Card */}
        <Card className="bg-background border border-destructive/20 shadow-sm">
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
                onClick={async (e) => {
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
              <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
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
                    <AlertDialogCancel>
                      {t('common.cancel', 'Cancel')}
                    </AlertDialogCancel>
                    <AlertDialogAction
                      onClick={async () => {
                        setConfirmOpen(false);
                        await onDelete();
                      }}
                    >
                      {t('common.delete', 'Delete')}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          </CardContent>
        </Card>

        {/* Factory Reset Card */}
        <Card className="bg-background border border-destructive/20 shadow-sm">
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
                onOpenChange={setResetConfirmOpen}
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
                  <AlertDialogFooter>
                    <AlertDialogCancel>
                      {t('common.cancel', 'Cancel')}
                    </AlertDialogCancel>
                    <AlertDialogAction onClick={onReset}>
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
