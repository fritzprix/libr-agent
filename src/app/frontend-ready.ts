import { emit } from '@tauri-apps/api/event';

import { getLogger } from '@/lib/logger';

const logger = getLogger('FrontendReady');

let hasEmittedFrontendReady = false;
let frontendReadyPromise: Promise<void> | null = null;

export async function emitFrontendReadyOnce(): Promise<void> {
  if (hasEmittedFrontendReady) {
    return;
  }

  if (frontendReadyPromise) {
    return frontendReadyPromise;
  }

  frontendReadyPromise = emit('frontend-ready')
    .then(() => {
      hasEmittedFrontendReady = true;
    })
    .catch((error: unknown) => {
      logger.error('Failed to emit frontend-ready event', error);
    })
    .finally(() => {
      frontendReadyPromise = null;
    });

  return frontendReadyPromise;
}

export function __resetFrontendReadyForTests() {
  hasEmittedFrontendReady = false;
  frontendReadyPromise = null;
}
