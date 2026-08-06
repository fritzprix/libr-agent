import { z } from 'zod';

const KNOWN_PROCESS_STATUSES = [
  'starting',
  'running',
  'finished',
  'failed',
  'killed',
] as const;

const KnownProcessStatusSchema = z.enum(KNOWN_PROCESS_STATUSES);

export type ProcessStatus = z.infer<typeof KnownProcessStatusSchema>;

/**
 * Accepts known statuses; unknown backend values fall back to `finished`
 * so a single unexpected status cannot fail the entire process list parse.
 * Uses .catch() for reliable fallback (Zod 3.21+).
 */
export const ProcessStatusSchema = KnownProcessStatusSchema.catch('finished');

export const ProcessEntrySchema = z.object({
  process_id: z.string(),
  name: z.string().nullish(),
  command: z.string(),
  status: ProcessStatusSchema,
  pid: z.number().nullish(),
  started_at: z.string(),
  exit_code: z.number().nullish(),
});

export type ProcessEntry = z.infer<typeof ProcessEntrySchema>;

export const ListProcessesResultSchema = z.object({
  processes: z.array(ProcessEntrySchema),
  total: z.number(),
  running: z.number(),
  finished: z.number(),
});

export type ListProcessesResult = z.infer<typeof ListProcessesResultSchema>;

const ProcessOutputStreamSchema = z
  .object({
    content: z.array(z.string()).catch([]),
    lines_returned: z.number().optional(),
    total_size_bytes: z.number().optional(),
  })
  .catch({
    content: [],
    lines_returned: undefined,
    total_size_bytes: undefined,
  });

export const ReadProcessOutputResultSchema = z.object({
  process_id: z.string(),
  stream: z.string(),
  mode: z.string(),
  status: z.string(),
  is_process_running: z.boolean().optional(),
  outputs: z
    .object({
      stdout: ProcessOutputStreamSchema.optional(),
      stderr: ProcessOutputStreamSchema.optional(),
    })
    .passthrough()
    .catch({}),
});

export type ReadProcessOutputResult = z.infer<
  typeof ReadProcessOutputResultSchema
>;

export function isActiveProcessStatus(status: ProcessStatus): boolean {
  return status === 'starting' || status === 'running';
}

export function parseListProcessesResult(
  value: unknown,
): ListProcessesResult | null {
  const parsed = ListProcessesResultSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}

export function parseReadProcessOutputResult(
  value: unknown,
): ReadProcessOutputResult | null {
  const parsed = ReadProcessOutputResultSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}
