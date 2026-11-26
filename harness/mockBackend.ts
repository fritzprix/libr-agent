// Mock of @/lib/backend for the #3068 OAuth screenshot harness.
// Behavior is driven by ?state=<inprogress|cancelled|timedOut|providerError|success>.

export const shotState =
  new URLSearchParams(window.location.search).get('state') ?? 'inprogress';

let pendingReject: (e: Error) => void = () => {};

export async function hasOAuthToken(_serverId: string): Promise<boolean> {
  return shotState === 'success';
}

export async function isOAuthFlowActive(_serverId: string): Promise<boolean> {
  return false;
}

export function startOAuthFlow(_serverId: string, _config: unknown): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    pendingReject = reject;
    if (shotState === 'timedOut') {
      setTimeout(
        () => reject(new Error('AUTH_TIMEOUT: OAuth authorization timed out after 5 minutes')),
        120,
      );
    } else if (shotState === 'providerError') {
      setTimeout(
        () =>
          reject(
            new Error('OAuth provider error (access_denied): The user cancelled the request'),
          ),
        120,
      );
    } else if (shotState === 'success') {
      setTimeout(() => resolve('authorized'), 120);
    }
    // inprogress / cancelled: stay pending until cancel (or never).
  });
}

export function cancelOAuthFlow(_serverId: string): Promise<string> {
  pendingReject(new Error('AUTH_CANCELLED: OAuth authorization cancelled'));
  return Promise.resolve('cancelled');
}

export function revokeOAuthToken(_serverId: string): Promise<string> {
  return Promise.resolve('revoked');
}
