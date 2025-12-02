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
  status: 'pending' | 'completed' | 'blocked';
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  dependsOn?: number[];
  order: number;
  createdAt: number;
}

export interface PlanningMemo {
  id?: number;
  sessionId: string;
  threadId: string;
  content: string;
  createdAt: number;
}

export class PlanningDatabase extends Dexie {
  goals!: Table<PlanningGoal>;
  todos!: Table<PlanningTodo>;
  memos!: Table<PlanningMemo>;

  constructor() {
    super('PlanningDatabase');
    this.version(1).stores({
      goals: '++id, [sessionId+threadId], isActive',
      todos: '++id, [sessionId+threadId], status',
      memos: '++id, [sessionId+threadId]',
    });
  }
}

export const db = new PlanningDatabase();
