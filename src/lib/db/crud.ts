import type { Assistant, Group, Message, Session } from '@/models/chat';
import type {
  CRUD,
  DatabaseObject,
  FileChunk,
  FileContent,
  FileContentCRUD,
  FileStore,
  Page,
} from './types';
import { LocalDatabase } from './service';

const createPage = <T>(
  items: T[],
  page: number,
  pageSize: number,
  totalItems: number,
): Page<T> => {
  if (pageSize === -1) {
    return {
      items,
      page: 1,
      pageSize: totalItems,
      totalItems,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  return {
    items,
    page,
    pageSize,
    totalItems,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
};

export const assistantsCRUD: CRUD<Assistant> = {
  upsert: async (assistant: Assistant) => {
    const now = new Date();
    if (!assistant.createdAt) assistant.createdAt = now;
    assistant.updatedAt = now;
    await LocalDatabase.getInstance().assistants.put(assistant);
  },
  upsertMany: async (assistants: Assistant[]) => {
    const now = new Date();
    const updatedAssistants = assistants.map((assistant) => ({
      ...assistant,
      createdAt: assistant.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().assistants.bulkPut(updatedAssistants);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().assistants.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().assistants.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Assistant>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.assistants.count();

    if (pageSize === -1) {
      const items = await db.assistants.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.assistants
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().assistants.count();
  },
};

export const objectsCRUD: CRUD<
  DatabaseObject<unknown>,
  DatabaseObject<unknown>
> = {
  upsert: async <T>(object: DatabaseObject<T>) => {
    const now = new Date();
    if (!object.createdAt) object.createdAt = now;
    object.updatedAt = now;
    await LocalDatabase.getInstance().objects.put(
      object as DatabaseObject<unknown>,
    );
  },
  upsertMany: async <T>(objects: DatabaseObject<T>[]) => {
    const now = new Date();
    const updatedObjects = objects.map((obj) => ({
      ...obj,
      createdAt: obj.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().objects.bulkPut(
      updatedObjects as DatabaseObject<unknown>[],
    );
  },
  read: async <T>(key: string): Promise<DatabaseObject<T> | undefined> => {
    return LocalDatabase.getInstance().objects.get(key) as Promise<
      DatabaseObject<T> | undefined
    >;
  },
  delete: async (key: string) => {
    await LocalDatabase.getInstance().objects.delete(key);
  },
  getPage: async <T>(
    page: number,
    pageSize: number,
  ): Promise<Page<DatabaseObject<T>>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.objects.count();

    if (pageSize === -1) {
      const items = (await db.objects
        .orderBy('createdAt')
        .toArray()) as DatabaseObject<T>[];
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = (await db.objects
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray()) as DatabaseObject<T>[];

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().objects.count();
  },
};

export const sessionsCRUD: CRUD<Session> = {
  upsert: async (session: Session) => {
    const now = new Date();
    if (!session.createdAt) session.createdAt = now;
    session.updatedAt = now;
    await LocalDatabase.getInstance().sessions.put(session);
  },
  upsertMany: async (sessions: Session[]) => {
    const now = new Date();
    const updatedSessions = sessions.map((session) => ({
      ...session,
      createdAt: session.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().sessions.bulkPut(updatedSessions);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().sessions.get(id);
  },
  delete: async (id: string) => {
    const db = LocalDatabase.getInstance();
    await db.transaction(
      'rw',
      [db.sessions, db.messages, db.fileStores, db.fileContents, db.fileChunks],
      async () => {
        await db.messages.where('sessionId').equals(id).delete();
        const stores = await db.fileStores
          .where('sessionId')
          .equals(id)
          .toArray();
        for (const store of stores) {
          const contents = await db.fileContents
            .where('storeId')
            .equals(store.id)
            .toArray();
          for (const content of contents) {
            await db.fileChunks.where('contentId').equals(content.id).delete();
          }
          await db.fileContents.where('storeId').equals(store.id).delete();
        }
        await db.fileStores.where('sessionId').equals(id).delete();
        await db.sessions.delete(id);
      },
    );
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Session>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.sessions.count();

    if (pageSize === -1) {
      const items = await db.sessions.orderBy('updatedAt').reverse().toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.sessions
      .orderBy('updatedAt')
      .reverse()
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().sessions.count();
  },
};

export const messagesCRUD: CRUD<Message> = {
  upsert: async (message: Message) => {
    const now = new Date();
    if (!message.createdAt) message.createdAt = now;
    message.updatedAt = now;
    await LocalDatabase.getInstance().messages.put(message);
  },
  upsertMany: async (messages: Message[]) => {
    const now = new Date();
    const updatedMessages = messages.map((msg) => ({
      ...msg,
      createdAt: msg.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().messages.bulkPut(updatedMessages);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().messages.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().messages.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Message>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.messages.count();

    if (pageSize === -1) {
      const items = await db.messages.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.messages
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().messages.count();
  },
};

export const groupsCRUD: CRUD<Group> = {
  upsert: async (group: Group) => {
    const now = new Date();
    if (!group.createdAt) group.createdAt = now;
    group.updatedAt = now;
    await LocalDatabase.getInstance().groups.put(group);
  },
  upsertMany: async (groups: Group[]) => {
    const now = new Date();
    const updatedGroups = groups.map((group) => ({
      ...group,
      createdAt: group.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().groups.bulkPut(updatedGroups);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().groups.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().groups.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Group>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.groups.count();

    if (pageSize === -1) {
      const items = await db.groups.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.groups
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().groups.count();
  },
};

export const fileStoresCRUD: CRUD<FileStore> = {
  upsert: async (store: FileStore) => {
    const now = new Date();
    if (!store.createdAt) store.createdAt = now;
    store.updatedAt = now;
    await LocalDatabase.getInstance().fileStores.put(store);
  },
  upsertMany: async (stores: FileStore[]) => {
    const now = new Date();
    const updatedStores = stores.map((store) => ({
      ...store,
      createdAt: store.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().fileStores.bulkPut(updatedStores);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().fileStores.get(id);
  },
  delete: async (id: string) => {
    const db = LocalDatabase.getInstance();
    await db.transaction(
      'rw',
      db.fileStores,
      db.fileContents,
      db.fileChunks,
      async () => {
        const contents = await db.fileContents
          .where('storeId')
          .equals(id)
          .toArray();
        for (const content of contents) {
          await db.fileChunks.where('contentId').equals(content.id).delete();
        }
        await db.fileContents.where('storeId').equals(id).delete();
        await db.fileStores.delete(id);
      },
    );
  },
  getPage: async (page: number, pageSize: number): Promise<Page<FileStore>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.fileStores.count();

    if (pageSize === -1) {
      const items = await db.fileStores.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.fileStores
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().fileStores.count();
  },
};

export const fileContentsCRUD: FileContentCRUD = {
  upsert: async (content: FileContent) => {
    await LocalDatabase.getInstance().fileContents.put(content);
  },
  upsertMany: async (contents: FileContent[]) => {
    await LocalDatabase.getInstance().fileContents.bulkPut(contents);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().fileContents.get(id);
  },
  delete: async (id: string) => {
    const db = LocalDatabase.getInstance();
    await db.transaction('rw', db.fileContents, db.fileChunks, async () => {
      await db.fileChunks.where('contentId').equals(id).delete();
      await db.fileContents.delete(id);
    });
  },
  getPage: async (
    page: number,
    pageSize: number,
  ): Promise<Page<FileContent>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.fileContents.count();

    if (pageSize === -1) {
      const items = await db.fileContents
        .orderBy('uploadedAt')
        .reverse()
        .toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.fileContents
      .orderBy('uploadedAt')
      .reverse()
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().fileContents.count();
  },
  findByHashAndStore: async (
    contentHash: string,
    storeId: string,
  ): Promise<FileContent | undefined> => {
    const db = LocalDatabase.getInstance();
    return await db.fileContents
      .where('[storeId+contentHash]')
      .equals([storeId, contentHash])
      .first();
  },
};

export const fileChunksCRUD: CRUD<FileChunk> = {
  upsert: async (chunk: FileChunk) => {
    await LocalDatabase.getInstance().fileChunks.put(chunk);
  },
  upsertMany: async (chunks: FileChunk[]) => {
    await LocalDatabase.getInstance().fileChunks.bulkPut(chunks);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().fileChunks.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().fileChunks.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<FileChunk>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.fileChunks.count();

    if (pageSize === -1) {
      const items = await db.fileChunks.orderBy('chunkIndex').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.fileChunks
      .orderBy('chunkIndex')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().fileChunks.count();
  },
};
