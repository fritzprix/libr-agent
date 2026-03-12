import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { getLogger } from '@/lib/logger';
import { useDnDContext } from '@/context/DnDContext';
import { importAssistantSkills } from '@/lib/backend/skills';
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Badge,
  Checkbox,
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
import { RefreshCw, Copy, Trash2, Upload } from 'lucide-react';
import { toast } from 'sonner';
import { useEditor } from '@/context/EditorContext';
import { Assistant } from '@/models/chat';
import { useAssistantSkills } from './hooks/useAssistantSkills';

const logger = getLogger('SkillsEditor');

export default function SkillsEditor() {
  const { t } = useTranslation('common');
  const { draft } = useEditor<Assistant>();
  const { subscribe } = useDnDContext();
  const cardRef = useRef<HTMLDivElement>(null);

  const [isDragging, setIsDragging] = useState(false);
  const [showResetDialog, setShowResetDialog] = useState(false);

  const {
    skills,
    isLoading,
    isResetting,
    loadingSkills,
    fetchSkills,
    handleOverride,
    handleRevert,
    handleToggle,
    confirmReset,
  } = useAssistantSkills();

  // Subscribe to DnD events using the centralized context
  useEffect(() => {
    if (!draft?.id || !cardRef.current) return;

    const unlisten = subscribe(
      cardRef,
      async (event, payload) => {
        if (event === 'drag-over') {
          setIsDragging(true);
        } else if (event === 'leave') {
          setIsDragging(false);
        } else if (
          event === 'drop' &&
          payload.paths &&
          payload.paths.length > 0
        ) {
          setIsDragging(false);
          const filePath = payload.paths[0];
          const toastId = toast.loading(t('skills.importing'));

          try {
            await importAssistantSkills(draft.id, filePath);
            toast.success(t('skills.importSuccess'), { id: toastId });
            fetchSkills();
          } catch (error) {
            logger.error('Failed to import skills:', error);
            toast.error(`${t('skills.importFailed')}: ${error}`, {
              id: toastId,
            });
          }
        }
      },
      { priority: 1 }, // Higher priority to capture events
    );

    return () => {
      unlisten();
    };
  }, [draft?.id, fetchSkills, subscribe, t]);

  const handleReset = () => {
    setShowResetDialog(true);
  };

  const hasAssistantSkills = skills.some((s) => s.source === 'assistant');

  if (!draft?.id) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{t('skills.title')}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {t('skills.saveFirst')}
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div ref={cardRef}>
      <Card
        className={isDragging ? 'border-primary ring-2 ring-primary/20' : ''}
      >
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div className="flex flex-col space-y-1">
            <CardTitle className="text-sm font-medium">
              {t('skills.title')}
            </CardTitle>
            <p className="text-xs text-muted-foreground">
              {t('skills.dragDropDesc')}
            </p>
          </div>

          <div className="flex items-center gap-2">
            {hasAssistantSkills && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleReset}
                title={t('skills.resetTooltip')}
                className="text-destructive border-destructive/50 hover:bg-destructive/10"
              >
                <Trash2 className="h-4 w-4 mr-2" />
                {t('skills.reset')}
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={fetchSkills}
              disabled={isLoading}
            >
              <RefreshCw
                className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`}
              />
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {isDragging && (
            <div className="absolute inset-0 z-50 bg-background/80 flex items-center justify-center rounded-lg border-2 border-dashed border-primary">
              <div className="flex flex-col items-center">
                <Upload className="h-10 w-10 text-primary mb-2" />
                <p className="text-lg font-medium">{t('skills.dropHere')}</p>
              </div>
            </div>
          )}

          <div className="space-y-2">
            {skills.length === 0 ? (
              <div className="text-sm text-muted-foreground text-center py-4">
                {t('skills.noSkills')}
              </div>
            ) : (
              skills.map((skill) => {
                const isDisabled = draft.disabledSkills?.includes(skill.name);
                return (
                  <div
                    key={skill.path.toString()}
                    className={`flex items-center justify-between p-2 rounded-md border ${isDisabled ? 'bg-muted/50 opacity-70' : 'bg-card'}`}
                  >
                    <div className="flex items-center gap-3">
                      <Checkbox
                        id={`skill-${skill.name}`}
                        checked={!isDisabled}
                        onCheckedChange={(checked) =>
                          handleToggle(skill.name, checked as boolean)
                        }
                      />
                      <div className="flex flex-col gap-1">
                        <div className="flex items-center gap-2">
                          <span
                            className={`font-medium ${isDisabled ? 'line-through text-muted-foreground' : ''}`}
                          >
                            {skill.name}
                          </span>
                          {skill.source === 'assistant' ? (
                            <Badge variant="secondary" className="text-xs">
                              {t('skills.sourceAssistant')}
                            </Badge>
                          ) : (
                            <Badge variant="outline" className="text-xs">
                              {t('skills.sourceGlobal')}
                            </Badge>
                          )}
                        </div>
                        <span className="text-xs text-muted-foreground">
                          {skill.description}
                        </span>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {skill.source === 'global' && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => handleOverride(skill.name)}
                          title={t('skills.override')}
                          disabled={isDisabled || loadingSkills[skill.name]}
                        >
                          {loadingSkills[skill.name] ? (
                            <RefreshCw className="h-4 w-4 animate-spin text-muted-foreground" />
                          ) : (
                            <Copy className="h-4 w-4" />
                          )}
                        </Button>
                      )}
                      {skill.source === 'assistant' && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => handleRevert(skill.name)}
                          title={t('skills.revert')}
                          disabled={isDisabled || loadingSkills[skill.name]}
                        >
                          {loadingSkills[skill.name] ? (
                            <RefreshCw className="h-4 w-4 animate-spin text-muted-foreground" />
                          ) : (
                            <Trash2 className="h-4 w-4 text-destructive" />
                          )}
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </CardContent>
      </Card>

      <AlertDialog
        open={showResetDialog}
        onOpenChange={(open) => !isResetting && setShowResetDialog(open)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('skills.resetTitle')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('skills.resetConfirm')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isResetting}>
              {t('common.cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void confirmReset(() => setShowResetDialog(false));
              }}
              disabled={isResetting}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {isResetting && (
                <RefreshCw className="w-3 h-3 mr-2 animate-spin" />
              )}
              {t('skills.reset')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
