import { useState, useEffect } from 'react';
import { useTranslation, Trans } from 'react-i18next';
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
import type { ScheduledTask } from '@/lib/backend/scheduled-tasks';
import { MentionTextarea } from './MentionTextarea';
import { ScheduleBuilder } from './ScheduleBuilder';
import { listAssistants } from '@/lib/backend/assistants';
import type { Assistant } from '@/models/chat';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ScheduledTaskModal');

interface ScheduledTaskModalProps {
  open: boolean;
  task?: ScheduledTask | null;
  onClose: () => void;
  onSave: (data: {
    name: string;
    cronExpression: string;
    assistantId: string;
    message: string;
  }) => Promise<void>;
}

export function ScheduledTaskModal({
  open,
  task,
  onClose,
  onSave,
}: ScheduledTaskModalProps) {
  const { t } = useTranslation();
  const [assistants, setAssistants] = useState<Assistant[]>([]);

  // Load assistants once
  useEffect(() => {
    listAssistants()
      .then(setAssistants)
      .catch((e: unknown) => logger.error('Failed to load assistants', e));
  }, []);

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
  }) => Promise<void>;
}

function ScheduledTaskForm({
  task,
  assistants,
  onClose,
  onSave,
}: ScheduledTaskFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(task?.name ?? '');
  const [cronExpression, setCronExpression] = useState(
    task?.cronExpression ?? '0 9 * * *',
  );
  const [assistantId, setAssistantId] = useState(
    task?.assistantId ?? assistants[0]?.id ?? '',
  );
  const [message, setMessage] = useState(task?.message ?? '');
  const [saving, setSaving] = useState(false);
  const [prevAssistants, setPrevAssistants] = useState(assistants);

  // Set default assistantId for new tasks if assistants finish loading after the form mounts
  if (assistants !== prevAssistants) {
    setPrevAssistants(assistants);
    if (!task && !assistantId && assistants.length > 0) {
      setAssistantId(assistants[0].id);
    }
  }

  const handleSave = async () => {
    if (
      !name.trim() ||
      !cronExpression.trim() ||
      !assistantId ||
      !message.trim()
    )
      return;
    setSaving(true);
    try {
      await onSave({
        name: name.trim(),
        cronExpression: cronExpression.trim(),
        assistantId,
        message: message.trim(),
      });
      onClose();
    } catch (e: unknown) {
      logger.error('Failed to save scheduled task', e);
    } finally {
      setSaving(false);
    }
  };

  const isValid = Boolean(
    name.trim() && cronExpression.trim() && assistantId && message.trim(),
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
          <Select value={assistantId} onValueChange={setAssistantId}>
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
        </div>

        {/* Human-readable schedule builder */}
        <div className="grid gap-1.5">
          <Label>{t('scheduledTasks.modal.scheduleLabel')}</Label>
          <ScheduleBuilder
            value={cronExpression}
            onChange={setCronExpression}
          />
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
            assistantId={assistantId}
            rows={3}
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
