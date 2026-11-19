/**
 * Native dialog wrapper with an E2E picker override (#2903).
 *
 * Native file dialogs block unattended harnesses. The debug E2E control
 * plane can arm a one-shot override (`POST /api/e2e-control/picker`); the
 * next dialog resolves with it instead of waiting for a human. This wrapper
 * consumes the override for frontend `open()` calls; the Rust save path
 * consumes it inside `save_bytes_via_dialog`.
 *
 * The probe command exists in every build and always answers `null` unless
 * the debug control plane armed an override, so production behavior is
 * unchanged: the real dialog opens. Failures are deliberate and loud: the
 * probe rides the same Tauri IPC as the dialog itself, so if the probe call
 * or its payload validation rejects, the dialog call rejects too (the
 * caller's existing catch surfaces it as a toast) rather than silently
 * opening a dialog the harness can never answer.
 */
import { open, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { z } from 'zod';

import { safeInvoke } from '@/lib/backend/core';

const PickerOverrideSchema = z.union([
  z.object({ action: z.literal('path'), path: z.string().min(1) }),
  z.object({ action: z.literal('cancel') }),
  z.null(),
]);

type PickerOverride = z.infer<typeof PickerOverrideSchema>;

async function takePickerOverride(): Promise<PickerOverride> {
  const raw = await safeInvoke<unknown>('e2e_take_picker_override');
  return PickerOverrideSchema.parse(raw);
}

/**
 * Drop-in replacement for plugin-dialog's `open()`. If the E2E control plane
 * armed a picker override, resolve with it (cancel → null, path → the path
 * or `[path]` when `multiple`); otherwise show the real dialog.
 */
export async function openDialog(
  options?: OpenDialogOptions,
): Promise<string | string[] | null> {
  const override = await takePickerOverride();
  if (!override) {
    return open(options);
  }
  if (override.action === 'cancel') {
    return null;
  }
  return options?.multiple ? [override.path] : override.path;
}
