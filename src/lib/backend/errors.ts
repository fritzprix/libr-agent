/** Stable prefix returned by the Rust agent layer for Docker health-check failures. */
export const DOCKER_NOT_AVAILABLE_PREFIX = 'DOCKER_NOT_AVAILABLE:' as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** Extracts a human-readable message from backend/Tauri invoke errors. */
export function getBackendErrorMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (isRecord(error)) {
    const message = error.message;
    if (typeof message === 'string') {
      return message;
    }
  }
  return String(error);
}

/** Removes a structured error-code prefix when present. */
export function stripErrorCodePrefix(message: string, prefix: string): string {
  return message.startsWith(prefix)
    ? message.slice(prefix.length).trim()
    : message;
}

/** Detects Docker unavailability errors from structured or legacy string payloads. */
export function isDockerNotAvailableError(error: unknown): boolean {
  const message = getBackendErrorMessage(error);
  return (
    message.includes(DOCKER_NOT_AVAILABLE_PREFIX) ||
    message.includes('Docker is not available')
  );
}

/** Returns the display message for a Docker error, without the machine-readable prefix. */
export function getDockerNotAvailableMessage(error: unknown): string {
  const message = getBackendErrorMessage(error);
  return stripErrorCodePrefix(message, DOCKER_NOT_AVAILABLE_PREFIX);
}
