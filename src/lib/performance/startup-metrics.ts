import { getLogger } from '@/lib/logger';

const logger = getLogger('StartupMetrics');
const REQUIRED_MILESTONES = [
  'bootstrap-script-start',
  'app-mounted',
  'first-route-mounted',
  'settings-settled',
  'session-list-settled',
] as const;

export interface StartupMilestoneEntry {
  name: string;
  atMs: number;
  detail?: string;
}

export interface StartupMeasureEntry {
  name: string;
  startMilestone: string;
  endMilestone: string;
  durationMs: number;
}

export interface StartupIpcCallEntry {
  cmd: string;
  durationMs: number;
  ok: boolean;
}

export interface StartupIpcCommandSummary {
  cmd: string;
  count: number;
  failedCount: number;
  totalDurationMs: number;
  maxDurationMs: number;
}

export interface StartupIpcSummary {
  count: number;
  failedCount: number;
  totalDurationMs: number;
  maxDurationMs: number;
  commands: StartupIpcCommandSummary[];
}

export interface StartupLongTaskEntry {
  name: string;
  startMs: number;
  durationMs: number;
}

export interface StartupLongTaskSummary {
  count: number;
  totalDurationMs: number;
  maxDurationMs: number;
}

export interface StartupReport {
  generatedAt: string;
  milestones: StartupMilestoneEntry[];
  measures: StartupMeasureEntry[];
  ipc: StartupIpcSummary;
  longTasks: StartupLongTaskSummary;
}

interface StartupMetricsState {
  finalized: boolean;
  milestones: Map<string, StartupMilestoneEntry>;
  ipcCalls: StartupIpcCallEntry[];
  longTasks: StartupLongTaskEntry[];
  report: StartupReport | null;
  observer: PerformanceObserver | null;
}

declare global {
  interface Window {
    __LIBRAGENT_STARTUP_REPORT__?: StartupReport;
  }
}

const state: StartupMetricsState = {
  finalized: false,
  milestones: new Map<string, StartupMilestoneEntry>(),
  ipcCalls: [],
  longTasks: [],
  report: null,
  observer: null,
};

function nowMs(): number {
  if (typeof performance !== 'undefined') {
    return performance.now();
  }

  return Date.now();
}

function summarizeLongTasks(
  longTasks: readonly StartupLongTaskEntry[],
): StartupLongTaskSummary {
  return longTasks.reduce<StartupLongTaskSummary>(
    (summary, task) => ({
      count: summary.count + 1,
      totalDurationMs: summary.totalDurationMs + task.durationMs,
      maxDurationMs: Math.max(summary.maxDurationMs, task.durationMs),
    }),
    {
      count: 0,
      totalDurationMs: 0,
      maxDurationMs: 0,
    },
  );
}

function summarizeIpcCalls(
  ipcCalls: readonly StartupIpcCallEntry[],
): StartupIpcSummary {
  const byCommand = new Map<string, StartupIpcCommandSummary>();
  let globalFailedCount = 0;
  let globalTotalDurationMs = 0;
  let globalMaxDurationMs = 0;

  for (const ipcCall of ipcCalls) {
    globalFailedCount += ipcCall.ok ? 0 : 1;
    globalTotalDurationMs += ipcCall.durationMs;
    globalMaxDurationMs = Math.max(globalMaxDurationMs, ipcCall.durationMs);

    const existing = byCommand.get(ipcCall.cmd);
    if (existing) {
      existing.count += 1;
      existing.failedCount += ipcCall.ok ? 0 : 1;
      existing.totalDurationMs += ipcCall.durationMs;
      existing.maxDurationMs = Math.max(
        existing.maxDurationMs,
        ipcCall.durationMs,
      );
      continue;
    }

    byCommand.set(ipcCall.cmd, {
      cmd: ipcCall.cmd,
      count: 1,
      failedCount: ipcCall.ok ? 0 : 1,
      totalDurationMs: ipcCall.durationMs,
      maxDurationMs: ipcCall.durationMs,
    });
  }

  return {
    count: ipcCalls.length,
    failedCount: globalFailedCount,
    totalDurationMs: globalTotalDurationMs,
    maxDurationMs: globalMaxDurationMs,
    commands: Array.from(byCommand.values()).sort(
      (left, right) => right.totalDurationMs - left.totalDurationMs,
    ),
  };
}

function buildMeasure(
  milestonesByName: ReadonlyMap<string, StartupMilestoneEntry>,
  name: string,
  startMilestone: string,
  endMilestone: string,
): StartupMeasureEntry | null {
  const start = milestonesByName.get(startMilestone);
  const end = milestonesByName.get(endMilestone);

  if (!start || !end || end.atMs < start.atMs) {
    return null;
  }

  return {
    name,
    startMilestone,
    endMilestone,
    durationMs: end.atMs - start.atMs,
  };
}

export function createStartupReport({
  milestones,
  ipcCalls,
  longTasks,
}: {
  milestones: readonly StartupMilestoneEntry[];
  ipcCalls: readonly StartupIpcCallEntry[];
  longTasks: readonly StartupLongTaskEntry[];
}): StartupReport {
  const sortedMilestones = [...milestones].sort(
    (left, right) => left.atMs - right.atMs,
  );
  const milestonesByName = new Map(
    sortedMilestones.map((milestone) => [milestone.name, milestone] as const),
  );
  const measures = [
    buildMeasure(
      milestonesByName,
      'bootstrap-to-app-mounted',
      'bootstrap-script-start',
      'app-mounted',
    ),
    buildMeasure(
      milestonesByName,
      'bootstrap-to-first-route',
      'bootstrap-script-start',
      'first-route-mounted',
    ),
    buildMeasure(
      milestonesByName,
      'bootstrap-to-settings-settled',
      'bootstrap-script-start',
      'settings-settled',
    ),
    buildMeasure(
      milestonesByName,
      'bootstrap-to-session-list-settled',
      'bootstrap-script-start',
      'session-list-settled',
    ),
    buildMeasure(
      milestonesByName,
      'app-mounted-to-first-route',
      'app-mounted',
      'first-route-mounted',
    ),
    buildMeasure(
      milestonesByName,
      'first-route-to-session-list-settled',
      'first-route-mounted',
      'session-list-settled',
    ),
  ].flatMap((measure) => (measure ? [measure] : []));

  return {
    generatedAt: new Date().toISOString(),
    milestones: sortedMilestones,
    measures,
    ipc: summarizeIpcCalls(ipcCalls),
    longTasks: summarizeLongTasks(longTasks),
  };
}

function ensureLongTaskObserver(): void {
  if (state.observer || typeof window === 'undefined') {
    return;
  }

  if (typeof PerformanceObserver === 'undefined') {
    return;
  }

  const supportedEntryTypes = Array.isArray(
    PerformanceObserver.supportedEntryTypes,
  )
    ? PerformanceObserver.supportedEntryTypes
    : [];

  if (!supportedEntryTypes.includes('longtask')) {
    return;
  }

  try {
    state.observer = new PerformanceObserver((entryList) => {
      if (state.finalized) {
        return;
      }

      for (const entry of entryList.getEntries()) {
        state.longTasks.push({
          name: entry.name,
          startMs: entry.startTime,
          durationMs: entry.duration,
        });
      }
    });
    state.observer.observe({ entryTypes: ['longtask'] });
  } catch (error) {
    logger.warn('Failed to start long task observer', error);
    state.observer = null;
  }
}

function finalizeStartupReportIfReady(): void {
  if (state.finalized) {
    return;
  }

  const hasAllRequiredMilestones = REQUIRED_MILESTONES.every((milestone) =>
    state.milestones.has(milestone),
  );
  if (!hasAllRequiredMilestones) {
    return;
  }

  state.finalized = true;
  state.report = createStartupReport({
    milestones: Array.from(state.milestones.values()),
    ipcCalls: state.ipcCalls,
    longTasks: state.longTasks,
  });

  if (state.observer) {
    state.observer.disconnect();
    state.observer = null;
  }

  if (typeof window !== 'undefined') {
    window.__LIBRAGENT_STARTUP_REPORT__ = state.report;
    window.dispatchEvent(
      new CustomEvent<StartupReport>('libragent:startup-report', {
        detail: state.report,
      }),
    );
  }

  logger.info('Startup performance baseline captured', state.report);
}

export function markStartupMilestone(
  name: string,
  detail?: string,
): StartupMilestoneEntry {
  const existing = state.milestones.get(name);
  if (existing) {
    return existing;
  }

  ensureLongTaskObserver();

  const milestone: StartupMilestoneEntry = {
    name,
    atMs: nowMs(),
    ...(detail ? { detail } : {}),
  };
  state.milestones.set(name, milestone);

  if (
    typeof performance !== 'undefined' &&
    typeof performance.mark === 'function'
  ) {
    performance.mark(`libragent:${name}`);
  }

  finalizeStartupReportIfReady();
  return milestone;
}

export function recordStartupIpcCall(
  cmd: string,
  durationMs: number,
  ok: boolean,
): void {
  if (state.finalized) {
    return;
  }

  state.ipcCalls.push({ cmd, durationMs, ok });
}

export function getStartupReport(): StartupReport | null {
  return state.report;
}
