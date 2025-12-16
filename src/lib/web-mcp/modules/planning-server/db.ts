import Dexie, { type Table } from 'dexie';

export interface PlanningGoal {
  id?: number;
  sessionId: string;
  threadId: string;
  content: string;
  isActive: number; // 0 or 1, as boolean is not a valid IDB key
  createdAt: number;
}

export interface PlanningTodo {
  id?: number;
  sessionId: string;
  threadId: string;
  name: string;
  checked: boolean;
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  dependsOn?: number[];
  order: number;
  createdAt: number;
}

export interface PlanningScratchpadItem {
  id?: number;
  sessionId: string;
  threadId: string;
  content: string;
  source?: string;
  createdAt: number;
}

export class PlanningDatabase extends Dexie {
  goals!: Table<PlanningGoal>;
  todos!: Table<PlanningTodo>;
  scratchpad!: Table<PlanningScratchpadItem>;

  constructor() {
    super('PlanningDatabase');
    this.version(1).stores({
      goals: '++id, [sessionId+threadId], isActive',
      todos: '++id, [sessionId+threadId], status',
      scratchpad: '++id, [sessionId+threadId]',
    });

    // Version 2: Migrate from status to checked
    this.version(2)
      .stores({
        goals: '++id, [sessionId+threadId], isActive',
        todos: '++id, [sessionId+threadId], checked',
        scratchpad: '++id, [sessionId+threadId]',
      })
      .upgrade(async (tx) => {
        const todos = await tx.table('todos').toArray();
        for (const todo of todos) {
          const oldTodo = todo as unknown as {
            status: 'pending' | 'completed' | 'blocked';
          };
          await tx.table('todos').update(todo.id!, {
            checked: oldTodo.status === 'completed',
          });
        }
      });
  }
}

export const db = new PlanningDatabase();
