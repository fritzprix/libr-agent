import { describe, expect, it, vi, beforeEach } from 'vitest';
import * as settings from '../settings';
import { safeInvoke } from '../core';
import type { DatabaseObject } from '@/lib/db/types';

vi.mock('../core', () => ({
  safeInvoke: vi.fn(),
}));

describe('settings', () => {
  beforeEach(() => {
    vi.mocked(safeInvoke).mockReset();
  });

  describe('setSetting', () => {
    it('calls safeInvoke with set_setting command', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      await settings.setSetting('my_key', 'my_val');
      expect(safeInvoke).toHaveBeenCalledWith('set_setting', { key: 'my_key', value: 'my_val' });
    });
  });

  describe('getSetting', () => {
    it('returns deserialized setting when found', async () => {
      const dto = {
        key: 'test_key',
        value: 'test_value',
        createdAt: 1000,
        updatedAt: 2000,
      };
      vi.mocked(safeInvoke).mockResolvedValueOnce(dto);

      const result = await settings.getSetting<string>('test_key');

      expect(safeInvoke).toHaveBeenCalledWith('get_setting', { key: 'test_key' });
      expect(result).toEqual({
        key: 'test_key',
        value: 'test_value',
        createdAt: new Date(1000),
        updatedAt: new Date(2000),
      });
    });

    it('returns undefined when setting not found', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(null);
      const result = await settings.getSetting<string>('missing_key');
      expect(safeInvoke).toHaveBeenCalledWith('get_setting', { key: 'missing_key' });
      expect(result).toBeUndefined();
    });
  });

  describe('deleteSetting', () => {
    it('calls safeInvoke with delete_setting command', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      await settings.deleteSetting('del_key');
      expect(safeInvoke).toHaveBeenCalledWith('delete_setting', { key: 'del_key' });
    });
  });

  describe('listSettings', () => {
    it('returns array of deserialized settings', async () => {
      const dtos = [
        { key: 'k1', value: 'v1', createdAt: 1000, updatedAt: 2000 },
        { key: 'k2', value: 'v2', createdAt: 3000, updatedAt: 4000 },
      ];
      vi.mocked(safeInvoke).mockResolvedValueOnce(dtos);

      const result = await settings.listSettings();

      expect(safeInvoke).toHaveBeenCalledWith('list_settings');
      expect(result).toEqual([
        { key: 'k1', value: 'v1', createdAt: new Date(1000), updatedAt: new Date(2000) },
        { key: 'k2', value: 'v2', createdAt: new Date(3000), updatedAt: new Date(4000) },
      ]);
    });
  });

  describe('getSettingsPage', () => {
    const dtos = [
      { key: 'k1', value: 'v1', createdAt: 1000, updatedAt: 2000 },
      { key: 'k2', value: 'v2', createdAt: 3000, updatedAt: 4000 },
      { key: 'k3', value: 'v3', createdAt: 5000, updatedAt: 6000 },
      { key: 'k4', value: 'v4', createdAt: 7000, updatedAt: 8000 },
    ];

    it('returns all items when pageSize is -1', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(dtos);
      const result = await settings.getSettingsPage<string>(1, -1);

      expect(safeInvoke).toHaveBeenCalledWith('list_settings');
      expect(result.items.length).toBe(4);
      expect(result.totalItems).toBe(4);
      expect(result.page).toBe(1);
      expect(result.pageSize).toBe(4);
      expect(result.totalPages).toBe(1);
      expect(result.hasNextPage).toBe(false);
      expect(result.hasPreviousPage).toBe(false);
    });

    it('paginates correctly for first page', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(dtos);
      const result = await settings.getSettingsPage<string>(1, 2);

      expect(result.items.length).toBe(2);
      expect(result.items[0].key).toBe('k1');
      expect(result.items[1].key).toBe('k2');
      expect(result.totalItems).toBe(4);
      expect(result.totalPages).toBe(2);
      expect(result.hasNextPage).toBe(true);
      expect(result.hasPreviousPage).toBe(false);
    });

    it('paginates correctly for second page', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(dtos);
      const result = await settings.getSettingsPage<string>(2, 2);

      expect(result.items.length).toBe(2);
      expect(result.items[0].key).toBe('k3');
      expect(result.items[1].key).toBe('k4');
      expect(result.totalItems).toBe(4);
      expect(result.totalPages).toBe(2);
      expect(result.hasNextPage).toBe(false);
      expect(result.hasPreviousPage).toBe(true);
    });

    it('handles empty results', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce([]);
      const result = await settings.getSettingsPage<string>(1, 10);

      expect(result.items.length).toBe(0);
      expect(result.totalItems).toBe(0);
      expect(result.totalPages).toBe(1);
      expect(result.hasNextPage).toBe(false);
      expect(result.hasPreviousPage).toBe(false);
    });
  });

  describe('upsertSetting', () => {
    it('calls safeInvoke with set_setting command using object key and value', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);
      const obj: DatabaseObject<string> = {
        key: 'up_key',
        value: 'up_val',
        createdAt: new Date(),
        updatedAt: new Date(),
      };
      await settings.upsertSetting(obj);
      expect(safeInvoke).toHaveBeenCalledWith('set_setting', { key: 'up_key', value: 'up_val' });
    });
  });

  describe('upsertSettings', () => {
    it('calls safeInvoke for each object in array', async () => {
      vi.mocked(safeInvoke).mockResolvedValue(undefined);
      const objs: DatabaseObject<string>[] = [
        { key: 'up_k1', value: 'up_v1', createdAt: new Date(), updatedAt: new Date() },
        { key: 'up_k2', value: 'up_v2', createdAt: new Date(), updatedAt: new Date() },
      ];

      await settings.upsertSettings(objs);

      expect(safeInvoke).toHaveBeenCalledTimes(2);
      expect(safeInvoke).toHaveBeenNthCalledWith(1, 'set_setting', { key: 'up_k1', value: 'up_v1' });
      expect(safeInvoke).toHaveBeenNthCalledWith(2, 'set_setting', { key: 'up_k2', value: 'up_v2' });
    });
  });
});
