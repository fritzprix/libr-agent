import { safeInvoke } from './core';

export interface PendingPairingInfo {
  id: string;
  name: string;
  expiresInSeconds: number;
}

export interface IssuedApiKeySummary {
  id: string;
  name: string;
  createdAt: string;
  revokedAt?: string | null;
}

/**
 * Checks if there is any pending Session API pairing request.
 */
export async function getPendingSessionApiPairing(): Promise<PendingPairingInfo | null> {
  return safeInvoke<PendingPairingInfo | null>(
    'get_pending_session_api_pairing',
  );
}

/**
 * Approves a pending pairing request by verifying the 6-character code (xxx-yyy).
 */
export async function approveSessionApiPairing(
  code: string,
): Promise<IssuedApiKeySummary> {
  return safeInvoke<IssuedApiKeySummary>('approve_session_api_pairing', {
    code,
  });
}

/**
 * Rejects or cancels the currently pending pairing request.
 */
export async function rejectSessionApiPairing(): Promise<void> {
  return safeInvoke<void>('reject_session_api_pairing');
}

/**
 * Lists all issued Session API keys with their names and revocation status.
 */
export async function listSessionApiKeys(): Promise<IssuedApiKeySummary[]> {
  return safeInvoke<IssuedApiKeySummary[]>('list_session_api_keys');
}

/**
 * Revokes/deletes an issued Session API key by ID.
 */
export async function revokeSessionApiKey(id: string): Promise<void> {
  return safeInvoke<void>('revoke_session_api_key', { id });
}

/**
 * Deletes an issued Session API key by ID (alias for revokeSessionApiKey).
 */
export async function deleteSessionApiKey(id: string): Promise<void> {
  return revokeSessionApiKey(id);
}
