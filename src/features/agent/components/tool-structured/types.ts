import { z } from 'zod';
import { parseBuiltinToolName } from '@/lib/tool-call-utils';

export const WriteFileChangesSchema = z.object({
  previous_lines: z.number().optional(),
  previous_bytes: z.number().optional(),
  new_lines: z.number().optional(),
  new_bytes: z.number().optional(),
  lines_added: z.number().optional(),
  lines_removed: z.number().optional(),
});

export const WriteFileResultSchema = z.object({
  path: z.string(),
  requested_path: z.string().optional(),
  path_adjusted: z.boolean().optional(),
  suffix: z.string().nullish(),
  mode: z.string().optional(),
  action: z.enum([
    'created',
    'overwritten',
    'appended',
    'created_alternate_path',
  ]),
  bytes_written: z.number().optional(),
  lines: z.number().optional(),
  file_exists_before: z.boolean().optional(),
  requested_path_existed: z.boolean().optional(),
  changes: WriteFileChangesSchema.optional(),
  unified_diff: z.string().optional(),
});

export type WriteFileResult = z.infer<typeof WriteFileResultSchema>;

export const StrReplaceResultSchema = z.object({
  path: z.string(),
  replacements: z.number(),
  unified_diff: z.string(),
});

export type StrReplaceResult = z.infer<typeof StrReplaceResultSchema>;

export const RunShellResultSchema = z.object({
  command: z.string(),
  exit_code: z.number(),
  stdout: z.string().optional().default(''),
  stderr: z.string().optional().default(''),
  status: z.string().optional(),
  duration_ms: z.number().optional(),
  execution_type: z.string().optional(),
  cwd: z.string().optional(),
});

export type RunShellResult = z.infer<typeof RunShellResultSchema>;

export function parseWriteFileResult(value: unknown): WriteFileResult | null {
  const parsed = WriteFileResultSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}

export function parseStrReplaceResult(value: unknown): StrReplaceResult | null {
  const parsed = StrReplaceResultSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}

export function parseRunShellResult(value: unknown): RunShellResult | null {
  const parsed = RunShellResultSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}

/** Canonical `service__tool` name used by the structured-result dispatcher. */
export function resolveStructuredToolKey(toolName: string): string {
  const parsed = parseBuiltinToolName(toolName);
  if (parsed) {
    return `${parsed.serviceId}__${parsed.toolName}`;
  }
  return toolName;
}
