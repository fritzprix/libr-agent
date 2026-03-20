import { useEffect, useRef, useState, type RefObject } from 'react';
import { useTranslation, Trans } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Zap, FolderOpen, Upload, X } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { MentionTextarea } from './MentionTextarea';
import { ScheduleBuilder } from './ScheduleBuilder';
import type { Assistant } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useDnDContext } from '@/context/DnDContext';
import {
  checkDroppedPathType,
  registerDroppedFiles,
} from '@/lib/backend/file-operations';

const logger = getLogger('ScheduledTaskModal');

interface ScheduledTaskModalProps {
  open: boolean;
  task?: ScheduledTask | null;
  assistants: Assistant[];
  onClose: () => void;
  onSave: (data: {
    name: string;
    cronExpression: string;
    assistantId: string;
    message: string;
    yoloMode: boolean;
    workspaceOverride: string | null;
  }) => Promise<void>;
}

export function ScheduledTaskModal({
  open,
  task,
  assistants,
  onClose,
  onSave,
}: ScheduledTaskModalProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {task
              ? t('scheduledTasks.modal.titleEdit')
              : t('scheduledTasks.modal.titleNew')}
          </DialogTitle>
        </DialogHeader>

        {open && (
          <ScheduledTaskForm
            key={task?.id || 'new'}
            task={task}
            assistants={assistants}
            onClose={onClose}
            onSave={onSave}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

interface ScheduledTaskFormProps {
  task?: ScheduledTask | null;
  assistants: Assistant[];
  onClose: () => void;
  onSave: (data: {
    name: string;
    cronExpression: string;
    assistantId: string;
    message: string;
    yoloMode: boolean;
    workspaceOverride: string | null;
  }) => Promise<void>;
}

function ScheduledTaskForm({
  task,
  assistants,
  onClose,
  onSave,
}: ScheduledTaskFormProps) {
  const { t } = useTranslation();
  const { subscribe } = useDnDContext();
  const [name, setName] = useState(task?.name ?? '');
  const [cronExpression, setCronExpression] = useState(
    task?.cronExpression ?? '0 9 * * *',
  );

  const [userSelectedAssistantId, setUserSelectedAssistantId] = useState<
    string | undefined
  >(undefined);

  const hasAssistant = (
    assistantId: string | undefined,
  ): assistantId is string =>
    Boolean(
      assistantId &&
      assistants.some((assistant) => assistant.id === assistantId),
    );
  const effectiveAssistantId = hasAssistant(userSelectedAssistantId)
    ? userSelectedAssistantId
    : hasAssistant(task?.assistantId)
      ? task.assistantId
      : assistants[0]?.id;
  const [message, setMessage] = useState(task?.message ?? '');
  const [yoloMode, setYoloMode] = useState(task?.yoloMode ?? false);
  const [workspaceOverride, setWorkspaceOverride] = useState<string | null>(
    task?.workspaceOverride ?? null,
  );
  const [workspaceDragState, setWorkspaceDragState] = useState<
    'none' | 'valid' | 'invalid'
  >('none');
  const [browsingWorkspace, setBrowsingWorkspace] = useState(false);
  const [saving, setSaving] = useState(false);
  const workspaceDropRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const processDroppedPaths = (paths: string[]) => {
      const run = async () => {
        try {
          await registerDroppedFiles(paths);
        } catch (error: unknown) {
          logger.error('Failed to register dropped workspace folder', error);
          toast.error(t('scheduledTasks.modal.workspaceRegisterFailed'));
          return;
        }

        for (const filePath of paths) {
          try {
            const pathType = await checkDroppedPathType(filePath);
            if (pathType === 'directory') {
              setWorkspaceOverride(filePath);
              return;
            }
          } catch (error: unknown) {
            logger.error('Failed to inspect dropped workspace path', {
              filePath,
              error,
            });
          }
        }

        toast.error(t('scheduledTasks.modal.workspaceDropFolderError'));
      };

      void run();
    };

    const unsubscribe = subscribe(
      workspaceDropRef as RefObject<HTMLElement>,
      (event, payload) => {
        if (event === 'drag-over') {
          setWorkspaceDragState(
            payload.paths && payload.paths.length > 0 ? 'valid' : 'invalid',
          );
          return;
        }

        if (event === 'leave') {
          setWorkspaceDragState('none');
          return;
        }

        setWorkspaceDragState('none');
        if (payload.paths && payload.paths.length > 0) {
          processDroppedPaths(payload.paths);
        }
      },
      { priority: 5 },
    );

    return () => {
      unsubscribe();
    };
  }, [subscribe, t]);

  const handleBrowseWorkspace = async () => {
    if (browsingWorkspace) return;
    setBrowsingWorkspace(true);

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('scheduledTasks.modal.workspaceBrowseTitle'),
      });

      if (selected && typeof selected === 'string') {
        setWorkspaceOverride(selected);
      }
    } catch (error: unknown) {
      logger.error('Failed to open workspace folder dialog', error);
      toast.error(t('scheduledTasks.modal.workspaceBrowseError'));
    } finally {
      setBrowsingWorkspace(false);
    }
  };

  const handleSave = async () => {
    if (
      !name.trim() ||
      !cronExpression.trim() ||
      !effectiveAssistantId ||
      !message.trim()
    )
      return;
    setSaving(true);
    try {
      await onSave({
        name: name.trim(),
        cronExpression: cronExpression.trim(),
        assistantId: effectiveAssistantId,
        message: message.trim(),
        yoloMode,
        workspaceOverride,
      });
      onClose();
    } catch (e: unknown) {
      logger.error('Failed to save scheduled task', e);
    } finally {
      setSaving(false);
    }
  };

  const isValid = Boolean(
    name.trim() &&
    cronExpression.trim() &&
    effectiveAssistantId &&
    message.trim(),
  );

  return (
    <>
      <div className="grid gap-4 py-2">
        {/* Task name */}
        <div className="grid gap-1.5">
          <Label htmlFor="task-name">
            {t('scheduledTasks.modal.nameLabel')}
          </Label>
          <Input
            id="task-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('scheduledTasks.modal.namePlaceholder')}
          />
        </div>

        {/* Assistant */}
        <div className="grid gap-1.5">
          <Label>{t('scheduledTasks.modal.assistantLabel')}</Label>
          <Select
            value={effectiveAssistantId}
            onValueChange={setUserSelectedAssistantId}
          >
            <SelectTrigger>
              <SelectValue
                placeholder={t('scheduledTasks.modal.assistantPlaceholder')}
              />
            </SelectTrigger>
            <SelectContent>
              {assistants.map((a) => (
                <SelectItem key={a.id} value={a.id}>
                  {a.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {assistants.length === 0 && (
            <p className="text-xs text-muted-foreground">
              {t(
                'scheduledTasks.modal.noAssistants',
                'Create an assistant before scheduling a task.',
              )}
            </p>
          )}
        </div>

        {/* Human-readable schedule builder */}
        <div className="grid gap-1.5">
          <Label>{t('scheduledTasks.modal.scheduleLabel')}</Label>
          <ScheduleBuilder
            value={cronExpression}
            onChange={setCronExpression}
          />
        </div>

        <div className="grid gap-1.5">
          <Label>{t('scheduledTasks.modal.workspaceLabel')}</Label>
          <div
            ref={workspaceDropRef}
            className={cn(
              'rounded-lg border border-dashed p-3 transition-colors',
              workspaceDragState === 'valid' && 'border-success bg-success/10',
              workspaceDragState === 'invalid' &&
                'border-destructive bg-destructive/10',
            )}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3 min-w-0">
                <div className="mt-0.5 rounded-md bg-primary/10 p-2">
                  <FolderOpen className="h-4 w-4 text-primary" />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium">
                    {workspaceOverride
                      ? t('scheduledTasks.modal.workspaceSelected')
                      : t('scheduledTasks.modal.workspaceOptional')}
                  </p>
                  <p
                    className={cn(
                      'mt-1 text-xs text-muted-foreground',
                      workspaceOverride && 'truncate',
                    )}
                    title={workspaceOverride ?? undefined}
                  >
                    {workspaceOverride
                      ? workspaceOverride
                      : t('scheduledTasks.modal.workspaceHint')}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => void handleBrowseWorkspace()}
                  disabled={browsingWorkspace}
                >
                  <Upload className="mr-1 h-3.5 w-3.5" />
                  {t('scheduledTasks.modal.workspaceBrowse')}
                </Button>
                {workspaceOverride && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => setWorkspaceOverride(null)}
                    aria-label={t('scheduledTasks.modal.workspaceClearAria')}
                    className="h-8 w-8"
                  >
                    <X className="h-4 w-4" />
                  </Button>
                )}
              </div>
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t('scheduledTasks.modal.workspaceDropHint')}
            </p>
          </div>
        </div>

        {/* Message with @mention support */}
        <div className="grid gap-1.5">
          <Label>{t('scheduledTasks.modal.messageLabel')}</Label>
          <p className="text-xs text-muted-foreground">
            <Trans i18nKey="scheduledTasks.modal.messageHint">
              Use <code className="font-mono">@playbook:</code> or{' '}
              <code className="font-mono">@skill:</code> for autocomplete
            </Trans>
          </p>
          <MentionTextarea
            value={message}
            onChange={setMessage}
            assistantId={effectiveAssistantId}
            workspacePath={workspaceOverride}
            rows={3}
          />
        </div>

        {/* YOLO Mode toggle */}
        <div className="flex items-center justify-between p-3 border rounded-lg bg-muted/30">
          <div className="space-y-0.5">
            <div className="flex items-center gap-2">
              <Zap
                size={14}
                className={
                  yoloMode
                    ? 'text-primary fill-primary'
                    : 'text-muted-foreground'
                }
              />
              <Label htmlFor="yolo-mode" className="text-sm font-medium">
                {t('scheduledTasks.modal.yoloModeLabel', 'YOLO Mode')}
              </Label>
            </div>
            <p className="text-xs text-muted-foreground">
              {t(
                'scheduledTasks.modal.yoloModeHint',
                'Execute all tools without requiring manual approval',
              )}
            </p>
          </div>
          <Switch
            id="yolo-mode"
            checked={yoloMode}
            onCheckedChange={setYoloMode}
          />
        </div>
      </div>

      <DialogFooter>
        <Button variant="ghost" onClick={onClose} disabled={saving}>
          {t('scheduledTasks.modal.cancel')}
        </Button>
        <Button onClick={handleSave} disabled={!isValid || saving}>
          {saving
            ? t('scheduledTasks.modal.saving')
            : task
              ? t('scheduledTasks.modal.saveChanges')
              : t('scheduledTasks.modal.createTask')}
        </Button>
      </DialogFooter>
    </>
  );
}
