import { describe, it, expect, vi, beforeEach } from 'vitest';
import { WebMCPProxy } from '../mcp-proxy';
import type { WebMCPNotification } from '../../mcp/web-worker/message';

describe('WebMCPProxy Notification', () => {
  let proxy: WebMCPProxy;
  let mockWorker: Worker;

  beforeEach(async () => {
    mockWorker = {
      postMessage: vi.fn((msg) => {
        // Auto-respond to ping for initialization
        if (msg.type === 'ping' && mockWorker.onmessage) {
          setTimeout(() => {
            mockWorker.onmessage!({
              data: { id: msg.id, result: 'pong' },
            } as MessageEvent);
          }, 0);
        }
      }),
      onmessage: null,
      terminate: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    } as unknown as Worker;

    proxy = new WebMCPProxy({ workerInstance: mockWorker });
    await proxy.initialize();
  });

  it('should register notification handler', () => {
    const handler = vi.fn();
    const unsubscribe = proxy.onNotify('test-event', handler);

    expect(unsubscribe).toBeInstanceOf(Function);
  });

  it('should call handler when notification received', async () => {
    const handler = vi.fn();
    proxy.onNotify('db-changed', handler);

    // Simulate worker notification
    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'db-changed',
      data: { entity: 'mcpServers' },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).toHaveBeenCalledWith({ entity: 'mcpServers' });
  });

  it('should support multiple handlers for same event', async () => {
    const handler1 = vi.fn();
    const handler2 = vi.fn();

    proxy.onNotify('test-event', handler1);
    proxy.onNotify('test-event', handler2);

    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'test-event',
      data: { value: 123 },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler1).toHaveBeenCalledWith({ value: 123 });
    expect(handler2).toHaveBeenCalledWith({ value: 123 });
  });

  it('should unsubscribe handler', async () => {
    const handler = vi.fn();
    const unsubscribe = proxy.onNotify('test-event', handler);

    unsubscribe();

    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'test-event',
      data: {},
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).not.toHaveBeenCalled();
  });

  it('should not interfere with RPC responses', async () => {
    const handler = vi.fn();
    proxy.onNotify('test-event', handler);

    // Simulate RPC response (not notification)
    const response = {
      jsonrpc: '2.0' as const,
      id: '123',
      result: { structuredContent: 'test' },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: response }));

    // Notification handler should not be called
    expect(handler).not.toHaveBeenCalled();
  });

  it('should clear handlers on cleanup', () => {
    const handler = vi.fn();
    proxy.onNotify('test-event', handler);

    proxy.cleanup();

    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'test-event',
      data: {},
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).not.toHaveBeenCalled();
  });

  it('should handle notification without data', () => {
    const handler = vi.fn();
    proxy.onNotify('simple-event', handler);

    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'simple-event',
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler).toHaveBeenCalledWith(undefined);
  });

  it('should not error when no handlers registered for notification', () => {
    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'unregistered-event',
      data: { test: 'data' },
    };

    // Should not throw
    expect(() => {
      mockWorker.onmessage!(
        new MessageEvent('message', { data: notification }),
      );
    }).not.toThrow();
  });

  it('should continue executing other handlers if one throws', () => {
    const handler1 = vi.fn(() => {
      throw new Error('Handler 1 error');
    });
    const handler2 = vi.fn();

    proxy.onNotify('test-event', handler1);
    proxy.onNotify('test-event', handler2);

    const notification: WebMCPNotification = {
      type: 'notify',
      notifyType: 'test-event',
      data: { value: 123 },
    };

    mockWorker.onmessage!(new MessageEvent('message', { data: notification }));

    expect(handler1).toHaveBeenCalledWith({ value: 123 });
    expect(handler2).toHaveBeenCalledWith({ value: 123 });
  });
});
