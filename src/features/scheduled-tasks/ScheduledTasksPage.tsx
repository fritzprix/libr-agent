import { useEffect, useState, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Plus, Pencil, Trash2, Clock } from 'lucide-react';
import {
  listScheduledTasks,
  createScheduledTask,
  updateScheduledTask,
  toggleScheduledTask,
  deleteScheduledTask,
  type ScheduledTask,
} from '@/lib/backend/scheduled-tasks';
import { ScheduledTaskModal } from './components/ScheduledTaskModal';
import { describeCron } from './components/ScheduleBuilder';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ScheduledTasksPage');

export function ScheduledTasksPage() {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);

  const load = useCallback(async () => {
    try {
      const result = await listScheduledTasks();
      setTasks(result);
    } catch (e: unknown) {
      logger.error('Failed to load scheduled tasks', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleCreate = async (data: {
    name: string;
    cronExpression: string;
    assistantId: string;
    message: string;
  }) => {
    const task = await createScheduledTask(data);
    setTasks((prev) => [...prev, task]);
  };

  const handleUpdate = async (data: {
    name: string;
    cronExpression: string;
    assistantId: string;
    message: string;
  }) => {
    if (!editingTask) return;
    const updated = await updateScheduledTask(editingTask.id, data);
    setTasks((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
  };

  const handleToggle = async (task: ScheduledTask) => {
    try {
      const updated = await toggleScheduledTask(task.id, !task.enabled);
      setTasks((prev) => prev.map((t) => (t.id === updated.id ? updated : t)));
    } catch (e: unknown) {
      logger.error('Failed to toggle task', e);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteScheduledTask(id);
      setTasks((prev) => prev.filter((t) => t.id !== id));
    } catch (e: unknown) {
      logger.error('Failed to delete task', e);
    }
  };

  const openCreate = () => {
    setEditingTask(null);
    setModalOpen(true);
  };

  const openEdit = (task: ScheduledTask) => {
    setEditingTask(task);
    setModalOpen(true);
  };

  const formatNextRun = (ms: number | null): string => {
    if (!ms) return '—';
    const d = new Date(ms);
    return d.toLocaleString();
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-32 text-muted-foreground">
        Loading…
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6 max-w-3xl mx-auto">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">Scheduled Tasks</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            Cron-backed tasks that trigger agent workflows automatically
          </p>
        </div>
        <Button onClick={openCreate} size="sm">
          <Plus className="w-4 h-4 mr-1" />
          New task
        </Button>
      </div>

      {tasks.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-16 border border-dashed rounded-lg text-muted-foreground">
          <Clock className="w-8 h-8 opacity-40" />
          <p className="text-sm">No scheduled tasks yet</p>
          <Button variant="outline" size="sm" onClick={openCreate}>
            Create your first task
          </Button>
        </div>
      ) : (
        <ul className="flex flex-col gap-3">
          {tasks.map((task) => (
            <li
              key={task.id}
              className="flex items-start gap-4 rounded-lg border border-border p-4 bg-card"
            >
              <Switch
                checked={task.enabled}
                onCheckedChange={() => void handleToggle(task)}
                className="mt-0.5 shrink-0"
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="font-medium truncate">{task.name}</span>
                  <Badge variant="secondary" className="text-xs shrink-0">
                    {describeCron(task.cronExpression)}
                  </Badge>
                  {!task.enabled && (
                    <Badge variant="outline" className="text-xs shrink-0">
                      disabled
                    </Badge>
                  )}
                </div>
                <p className="text-sm text-muted-foreground mt-1 line-clamp-2 break-all">
                  {task.message}
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  Next run: {formatNextRun(task.nextRunAt)}
                </p>
              </div>
              <div className="flex items-center gap-1 shrink-0">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => openEdit(task)}
                >
                  <Pencil className="w-4 h-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-destructive hover:text-destructive"
                  onClick={() => void handleDelete(task.id)}
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <ScheduledTaskModal
        open={modalOpen}
        task={editingTask}
        onClose={() => setModalOpen(false)}
        onSave={editingTask ? handleUpdate : handleCreate}
      />
    </div>
  );
}
