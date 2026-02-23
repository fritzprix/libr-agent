import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useDnDContext } from '@/context/DnDContext';
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
import { SkillMetadata } from '@/types/skills';

export default function SkillsEditor() {
  const { t } = useTranslation('common');
  const { draft, update } = useEditor<Assistant>();
  const { subscribe } = useDnDContext(); // Move to top level
  const cardRef = useRef<HTMLDivElement>(null);
  const [skills, setSkills] = useState<SkillMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [showResetDialog, setShowResetDialog] = useState(false);

  const fetchSkills = useCallback(async () => {
    if (!draft?.id) return;
    setIsLoading(true);
    try {
      const result = await invoke<SkillMetadata[]>('get_aggregated_skills', {
        assistantId: draft.id,
      });
      setSkills(result);
    } catch (error) {
      console.error('Failed to fetch skills:', error);
      toast.error(t('skills.fetchFailed', 'Failed to fetch skills'));
    } finally {
      setIsLoading(false);
    }
  }, [draft?.id, t]);

  useEffect(() => {
    fetchSkills();
  }, [fetchSkills]);

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
          const toastId = toast.loading(
            t('skills.importing', 'Importing skills...'),
          );

          try {
            await invoke<string>('import_assistant_skills', {
              assistantId: draft.id,
              filePath,
            });
            toast.success(
              t('skills.importSuccess', 'Skills imported successfully'),
              { id: toastId },
            );
            fetchSkills();
          } catch (error) {
            console.error('Failed to import skills:', error);
            toast.error(
              `${t('skills.importFailed', 'Failed to import skills')}: ${error}`,
              { id: toastId },
            );
          }
        }
      },
      { priority: 1 }, // Higher priority to capture events
    );

    return () => {
      unlisten();
    };
  }, [draft?.id, fetchSkills, subscribe, t]);

  const handleOverride = async (skillName: string) => {
    if (!draft?.id) return;
    try {
      await invoke<string>('copy_global_to_assistant', {
        assistantId: draft.id,
        skillName,
      });
      toast.success(
        t('skills.overrideSuccess', 'Skill overridden successfully'),
      );
      fetchSkills();
    } catch (error) {
      console.error('Failed to override skill:', error);
      toast.error(t('skills.overrideFailed', 'Failed to override skill'));
    }
  };

  const handleRevert = async (skillName: string) => {
    if (!draft?.id) return;
    try {
      await invoke<string>('delete_assistant_skill', {
        assistantId: draft.id,
        skillName,
      });
      toast.success(
        t('skills.revertSuccess', 'Skill reverted to global version'),
      );
      fetchSkills();
    } catch (error) {
      console.error('Failed to revert skill:', error);
      toast.error(t('skills.revertFailed', 'Failed to revert skill'));
    }
  };

  const handleToggle = (skillName: string, checked: boolean) => {
    update((draft) => {
      if (!draft.disabledSkills) draft.disabledSkills = [];

      if (checked) {
        // Enable: remove from disabledSkills
        draft.disabledSkills = draft.disabledSkills.filter(
          (name) => name !== skillName,
        );
      } else {
        // Disable: add to disabledSkills
        if (!draft.disabledSkills.includes(skillName)) {
          draft.disabledSkills.push(skillName);
        }
      }
    });
  };

  const handleReset = () => {
    setShowResetDialog(true);
  };

  const confirmReset = async () => {
    if (!draft?.id) return;

    try {
      await invoke<string>('reset_assistant_skills', { assistantId: draft.id });
      toast.success(
        t('skills.resetSuccess', 'Assistant skills reset successfully'),
      );
      fetchSkills();
      setShowResetDialog(false);
    } catch (error) {
      console.error('Failed to reset skills:', error);
      toast.error(t('skills.resetFailed', 'Failed to reset skills'));
    }
  };

  const hasAssistantSkills = skills.some((s) => s.source === 'assistant');

  if (!draft?.id) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{t('skills.title', 'Skills Management')}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {t(
              'skills.saveFirst',
              'Please save the assistant to manage skills.',
            )}
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
              {t('skills.title', 'Skills Management')}
            </CardTitle>
            <p className="text-xs text-muted-foreground">
              {t(
                'skills.dragDropDesc',
                'Drag and drop a zip file or folder to override skills.',
              )}
            </p>
          </div>

          <div className="flex items-center gap-2">
            {hasAssistantSkills && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleReset}
                title={t(
                  'skills.resetTooltip',
                  'Remove all assistant-specific skills',
                )}
                className="text-destructive border-destructive/50 hover:bg-destructive/10"
              >
                <Trash2 className="h-4 w-4 mr-2" />
                {t('skills.reset', 'Reset Override')}
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
                <p className="text-lg font-medium">
                  {t('skills.dropHere', 'Drop to import skills')}
                </p>
              </div>
            </div>
          )}

          <div className="space-y-2">
            {skills.length === 0 ? (
              <div className="text-sm text-muted-foreground text-center py-4">
                {t('skills.noSkills', 'No skills found.')}
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
                              {t('skills.sourceAssistant', 'Assistant')}
                            </Badge>
                          ) : (
                            <Badge variant="outline" className="text-xs">
                              {t('skills.sourceGlobal', 'Global')}
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
                          title={t(
                            'skills.override',
                            'Override for this assistant',
                          )}
                          disabled={isDisabled}
                        >
                          <Copy className="h-4 w-4" />
                        </Button>
                      )}
                      {skill.source === 'assistant' && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => handleRevert(skill.name)}
                          title={t('skills.revert', 'Revert to global')}
                          disabled={isDisabled}
                        >
                          <Trash2 className="h-4 w-4 text-destructive" />
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

      <AlertDialog open={showResetDialog} onOpenChange={setShowResetDialog}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('skills.resetTitle', 'Reset Skills Override?')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'skills.resetConfirm',
                'This will remove all assistant-specific skills and revert to global defaults. This action cannot be undone.',
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>
              {t('common.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={confirmReset}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {t('skills.reset', 'Reset Override')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
