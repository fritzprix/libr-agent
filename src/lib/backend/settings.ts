import { safeInvoke } from './core';
import type { DatabaseObject, Page } from '@/lib/db/types';

interface SettingDto {
  key: string;
  value: unknown; // JSON
  createdAt: number;
  updatedAt: number;
}

function deserializeSetting<T>(dto: SettingDto): DatabaseObject<T> {
  return {
    key: dto.key,
    value: dto.value as T,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  };
}

export async function setSetting(key: string, value: unknown): Promise<void> {
  await safeInvoke<void>('set_setting', { key, value });
}

export async function getSetting<T>(
  key: string,
): Promise<DatabaseObject<T> | undefined> {
  // Backend returns SettingDto or errors/null?
  // settings_commands.rs `get_setting` -> Result<Option<SettingDto>, String>
  const dto = await safeInvoke<SettingDto | null>('get_setting', { key });
  return dto ? deserializeSetting<T>(dto) : undefined;
}

export async function deleteSetting(key: string): Promise<void> {
  await safeInvoke<void>('delete_setting', { key });
}

export async function listSettings(): Promise<DatabaseObject<unknown>[]> {
  const dtos = await safeInvoke<SettingDto[]>('list_settings');
  return dtos.map(deserializeSetting);
}

export async function getSettingsPage<T>(
  page: number,
  pageSize: number,
): Promise<Page<DatabaseObject<T>>> {
  // No paging support in backend, simulation
  const allDtos = await safeInvoke<SettingDto[]>('list_settings');
  const all = allDtos.map((d) => deserializeSetting<T>(d));

  const totalItems = all.length;
  if (pageSize === -1) {
    return {
      items: all,
      page: 1,
      pageSize: totalItems,
      totalItems,
      totalPages: 1,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  const totalPages = Math.ceil(totalItems / pageSize) || 1;
  const start = (page - 1) * pageSize;
  const end = start + pageSize;
  const items = all.slice(start, end);

  return {
    items,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
}

export async function upsertSetting<T>(obj: DatabaseObject<T>): Promise<void> {
  await setSetting(obj.key, obj.value);
}

export async function upsertSettings<T>(
  objs: DatabaseObject<T>[],
): Promise<void> {
  for (const obj of objs) {
    await setSetting(obj.key, obj.value);
  }
}
