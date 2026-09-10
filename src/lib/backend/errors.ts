/** Stable prefix returned by the Rust agent layer for Docker health-check failures. */
export const DOCKER_NOT_AVAILABLE_PREFIX = 'DOCKER_NOT_AVAILABLE:' as const;
/** Stable prefix when the Docker CLI binary is missing from PATH. */
export const DOCKER_NOT_INSTALLED_PREFIX = 'DOCKER_NOT_INSTALLED:' as const;

export type DockerAvailabilityIssue = 'not-installed' | 'not-available';

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

/** Detects a missing Docker CLI (not installed / not on PATH). */
export function isDockerNotInstalledError(error: unknown): boolean {
  const message = getBackendErrorMessage(error);
  return (
    message.includes(DOCKER_NOT_INSTALLED_PREFIX) ||
    message.includes('Docker CLI is not installed')
  );
}

/**
 * Detects Docker unavailability errors from structured or legacy string payloads.
 * Includes a missing CLI — callers that need the narrower case should use
 * `isDockerNotInstalledError` first.
 */
export function isDockerNotAvailableError(error: unknown): boolean {
  const message = getBackendErrorMessage(error);
  return (
    isDockerNotInstalledError(error) ||
    message.includes(DOCKER_NOT_AVAILABLE_PREFIX) ||
    message.includes('Docker is not available')
  );
}

/** Classifies a Docker runtime failure as missing CLI vs daemon/runtime down. */
export function classifyDockerAvailabilityError(
  error: unknown,
): DockerAvailabilityIssue | null {
  if (isDockerNotInstalledError(error)) {
    return 'not-installed';
  }
  if (isDockerNotAvailableError(error)) {
    return 'not-available';
  }
  return null;
}

/** Returns the display message for a Docker error, without the machine-readable prefix. */
export function getDockerNotAvailableMessage(error: unknown): string {
  const message = getBackendErrorMessage(error);
  return stripErrorCodePrefix(
    stripErrorCodePrefix(message, DOCKER_NOT_INSTALLED_PREFIX),
    DOCKER_NOT_AVAILABLE_PREFIX,
  );
}
