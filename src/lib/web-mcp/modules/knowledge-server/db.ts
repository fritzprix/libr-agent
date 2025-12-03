import Dexie, { type Table } from 'dexie';

export interface KnowledgeItem {
  id?: number;
  assistantId: string;
  title: string;
  content: string;
  tags: string[];
  createdAt: number;
  updatedAt: number;
}

export class KnowledgeDatabase extends Dexie {
  knowledge!: Table<KnowledgeItem>;

  constructor() {
    super('KnowledgeDatabase');
    this.version(1).stores({
      knowledge: '++id, assistantId, *tags, createdAt, updatedAt',
    });
  }
}

export const db = new KnowledgeDatabase();
