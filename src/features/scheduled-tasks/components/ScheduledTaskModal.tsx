import { useState, useEffect } from 'react';
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
            {task ? 'Edit Scheduled Task' : 'New Scheduled Task'}
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
  const [name, setName] = useState(task?.name ?? '');
  const [cronExpression, setCronExpression] = useState(
    task?.cronExpression ?? '0 9 * * *',
  );
  const [assistantId, setAssistantId] = useState(
    task?.assistantId ?? assistants[0]?.id ?? '',
  );
  const [message, setMessage] = useState(task?.message ?? '');
  const [saving, setSaving] = useState(false);

  // Set default assistantId for new tasks if assistants finish loading after the form mounts
  useEffect(() => {
    if (!task && !assistantId && assistants.length > 0) {
      setAssistantId(assistants[0].id);
    }
  }, [task, assistantId, assistants]);

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

  const isValid =
    name.trim() && cronExpression.trim() && assistantId && message.trim();

  return (
    <>
      <div className="grid gap-4 py-2">
        {/* Task name */}
        <div className="grid gap-1.5">
          <Label htmlFor="task-name">Task name</Label>
          <Input
            id="task-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Daily standup summary"
          />
        </div>

        {/* Assistant */}
        <div className="grid gap-1.5">
          <Label>Assistant</Label>
          <Select value={assistantId} onValueChange={setAssistantId}>
            <SelectTrigger>
              <SelectValue placeholder="Select an assistant…" />
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
          <Label>Schedule</Label>
          <ScheduleBuilder
            value={cronExpression}
            onChange={setCronExpression}
          />
        </div>

        {/* Message with @mention support */}
        <div className="grid gap-1.5">
          <Label>Message</Label>
          <p className="text-xs text-muted-foreground">
            Use <code className="font-mono">@playbook:</code> or{' '}
            <code className="font-mono">@skill:</code> for autocomplete
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
          Cancel
        </Button>
        <Button onClick={handleSave} disabled={!isValid || saving}>
          {saving ? 'Saving…' : task ? 'Save changes' : 'Create task'}
        </Button>
      </DialogFooter>
    </>
  );
}
