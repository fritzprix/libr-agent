import {
  isActiveProcessStatus,
  type ProcessEntry,
  type ProcessStatus,
} from './types';

/** Shared poll interval for process list refresh and closed-panel attention. */
export const PROCESS_LIST_POLL_INTERVAL_MS = 2500;

export const PROCESS_MESSAGE_REFRESH_DEBOUNCE_MS = 500;

/** Killed entries stay in the backend registry but are hidden from the UI list. */
export function filterVisibleProcesses(
  processes: ProcessEntry[],
): ProcessEntry[] {
  return processes.filter((process) => process.status !== 'killed');
}

export function summarizeVisibleProcesses(processes: ProcessEntry[]): {
  total: number;
  running: number;
  finished: number;
} {
  let running = 0;
  let finished = 0;
  for (const process of processes) {
    if (isActiveProcessStatus(process.status)) {
      running += 1;
    } else {
      finished += 1;
    }
  }
  return { total: processes.length, running, finished };
}

export function processListFingerprint(
  processes: Array<{ process_id: string; status: ProcessStatus | string }>,
): string {
  return processes
    .map((process) => `${process.process_id}:${process.status}`)
    .sort()
    .join('|');
}
