import { describe, expect, it } from 'vitest';
import { createStartupReport } from '../startup-metrics';

describe('startup-metrics', () => {
  it('builds a sorted report with milestone measures and summaries', () => {
    const report = createStartupReport({
      milestones: [
        { name: 'session-list-settled', atMs: 52 },
        { name: 'bootstrap-script-start', atMs: 0 },
        { name: 'first-route-mounted', atMs: 28, detail: 'agent' },
        { name: 'app-mounted', atMs: 12 },
        { name: 'settings-settled', atMs: 40, detail: 'ready' },
      ],
      ipcCalls: [
        { cmd: 'agent_get_all_sessions', durationMs: 18, ok: true },
        { cmd: 'get_settings', durationMs: 11, ok: true },
        { cmd: 'agent_get_all_sessions', durationMs: 7, ok: false },
      ],
      longTasks: [
        { name: 'self', startMs: 5, durationMs: 71 },
        { name: 'self', startMs: 90, durationMs: 55 },
      ],
    });

    expect(report.milestones.map((milestone) => milestone.name)).toEqual([
      'bootstrap-script-start',
      'app-mounted',
      'first-route-mounted',
      'settings-settled',
      'session-list-settled',
    ]);
    expect(report.measures).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: 'bootstrap-to-session-list-settled',
          durationMs: 52,
        }),
        expect.objectContaining({
          name: 'app-mounted-to-first-route',
          durationMs: 16,
        }),
      ]),
    );
    expect(report.ipc).toMatchObject({
      count: 3,
      failedCount: 1,
      totalDurationMs: 36,
      maxDurationMs: 18,
    });
    expect(report.ipc.commands[0]).toMatchObject({
      cmd: 'agent_get_all_sessions',
      count: 2,
      failedCount: 1,
      totalDurationMs: 25,
      maxDurationMs: 18,
    });
    expect(report.longTasks).toEqual({
      count: 2,
      totalDurationMs: 126,
      maxDurationMs: 71,
    });
  });
});
